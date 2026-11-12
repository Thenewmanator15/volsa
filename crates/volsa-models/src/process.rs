//! A helper program started in a Windows job object, so that a plugin's network runs apart from
//! Volsa: the job ends the helper when Volsa ends or the job is closed, lets it start no process
//! of its own, ends it rather than show a crash window, and, when a cap is given, refuses it
//! memory past the cap. The job also tells how much memory the helper held at most.
//!
//! The helper is put in the job before this module hands it back, and is expected to do nothing
//! until its first message arrives, so nothing it does runs outside the job.
//!
//! Every call into the system's job objects is marked unsafe, and each carries a `SAFETY`
//! comment. Elsewhere than on Windows there are no job objects: the helper is only killed on
//! stop and drop, and has no cap.

use std::io;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};

/// A helper program, its pipes, and the job it runs in.
#[derive(Debug)]
pub struct Helper {
    child: Child,
    #[cfg(windows)]
    job: job::Job,
}

/// Starts `program` with `args`, its input and output piped and its errors let go, with no
/// window, in a job object that caps its memory at `cap` bytes when one is given.
///
/// # Errors
///
/// When the program cannot be started, or cannot be put in its job; the program is then ended.
pub fn start_helper(program: &Path, args: &[&str], cap: Option<u64>) -> io::Result<Helper> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// Windows' flag for a process started with no console window.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let child = command.spawn()?;
    #[cfg(windows)]
    {
        let mut child = child;
        match job::Job::around(&child, cap) {
            Ok(job) => Ok(Helper { child, job }),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(error)
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = cap;
        Ok(Helper { child })
    }
}

impl Helper {
    /// The helper's input and output, the first time they are asked for.
    pub fn take_pipes(&mut self) -> Option<(ChildStdin, ChildStdout)> {
        Some((self.child.stdin.take()?, self.child.stdout.take()?))
    }

    /// Ends the helper at once: on a graphics card too, whatever it was handed.
    pub fn stop(&mut self) {
        #[cfg(windows)]
        self.job.end();
        let _ = self.child.kill();
    }

    /// Waits for the helper to end, and says how it ended.
    ///
    /// # Errors
    ///
    /// When the system cannot say.
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }

    /// The most memory the helper held at any moment, in bytes, as its job counts it: `None`
    /// where that cannot be read.
    pub fn peak_memory(&self) -> Option<u64> {
        #[cfg(windows)]
        {
            self.job.peak()
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.stop();
        let _ = self.child.wait();
    }
}

#[cfg(windows)]
mod job {
    //! The job object itself.

    use std::ffi::c_void;
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;

    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
        JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
        TerminateJobObject,
    };
    use windows::core::PCWSTR;

    /// The size of the limits handed to and read from the system: under 200 bytes.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a record of under 200 bytes"
    )]
    const LIMITS_SIZE: u32 = size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;

    /// A job object, closed when it is dropped: closing it ends every process in it.
    #[derive(Debug)]
    pub(super) struct Job(HANDLE);

    // SAFETY: a job object's handle may be used and closed from any thread.
    unsafe impl Send for Job {}
    // SAFETY: as above; the system serialises what is done to a job object.
    unsafe impl Sync for Job {}

    impl Job {
        /// A new job with the limits, holding `child`.
        pub(super) fn around(child: &Child, cap: Option<u64>) -> io::Result<Self> {
            // SAFETY: no attributes and no name are given: an unnamed job with default
            // security. The handle is owned by the `Job` made from it, which closes it once.
            let handle =
                unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(io::Error::other)?;
            let job = Job(handle);
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            let basic = &mut limits.BasicLimitInformation;
            basic.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
                | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
            basic.ActiveProcessLimit = 1;
            if let Some(cap) = cap {
                basic.LimitFlags |= JOB_OBJECT_LIMIT_PROCESS_MEMORY;
                limits.ProcessMemoryLimit = usize::try_from(cap).unwrap_or(usize::MAX);
            }
            let pointer: *const c_void = (&raw const limits).cast();
            // SAFETY: the job handle is open; the pointer is to a whole extended-limits record
            // that lives across the call, and the size given is that record's.
            unsafe {
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    pointer,
                    LIMITS_SIZE,
                )
            }
            .map_err(io::Error::other)?;
            let process = HANDLE(child.as_raw_handle());
            // SAFETY: both handles are open: the job's is owned here, and the process's belongs
            // to `child`, which outlives the call. The process is not closed by the call.
            unsafe { AssignProcessToJobObject(job.0, process) }.map_err(io::Error::other)?;
            Ok(job)
        }

        /// Ends every process in the job.
        pub(super) fn end(&self) {
            // SAFETY: the job handle is open until the `Job` is dropped. Ending a job whose
            // process has ended already does nothing.
            let _ = unsafe { TerminateJobObject(self.0, 1) };
        }

        /// The most memory any process of the job held at once, in bytes.
        pub(super) fn peak(&self) -> Option<u64> {
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            let pointer: *mut c_void = (&raw mut limits).cast();
            // SAFETY: the job handle is open; the pointer is to a whole extended-limits record
            // to be written, and the size given is that record's. No length is asked back.
            let read = unsafe {
                QueryInformationJobObject(
                    Some(self.0),
                    JobObjectExtendedLimitInformation,
                    pointer,
                    LIMITS_SIZE,
                    None,
                )
            };
            read.ok()?;
            u64::try_from(limits.PeakProcessMemoryUsed).ok()
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: the handle was made by `CreateJobObjectW` for this `Job` alone and is
            // closed only here, once.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
}
