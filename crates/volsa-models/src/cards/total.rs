//! How much memory the computer has in all, for a default memory budget. On Windows it is the
//! same call as [`super::free_memory`]'s, read for another figure.

/// The computer's physical memory, in bytes, where the system says; `None` where it cannot
/// be read.
pub fn total_memory() -> Option<u64> {
    system::total_memory()
}

/// The memory Linux says the computer has, in bytes, from the text of its `/proc/meminfo`:
/// the line `MemTotal`, which is in kibibytes. `None` when there is no such line or it holds
/// no such figure.
#[cfg_attr(all(windows, not(test)), allow(dead_code))]
pub(crate) fn total_in(meminfo: &str) -> Option<u64> {
    let line = meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))?;
    let kibibytes: u64 = line.trim().strip_suffix("kB")?.trim().parse().ok()?;
    kibibytes.checked_mul(1024)
}

#[cfg(windows)]
mod system {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    pub(super) fn total_memory() -> Option<u64> {
        let mut status = MEMORYSTATUSEX {
            // The call reads the length first, to know the structure it is given.
            dwLength: u32::try_from(size_of::<MEMORYSTATUSEX>()).unwrap_or_default(),
            ..MEMORYSTATUSEX::default()
        };
        // SAFETY: `status` is a plain structure of ours that outlives the call, with its
        // length set as the call asks. The call reads that length and writes the rest.
        unsafe { GlobalMemoryStatusEx(&raw mut status) }
            .ok()
            .map(|()| status.ullTotalPhys)
    }
}

#[cfg(not(windows))]
mod system {
    use super::total_in;

    pub(super) fn total_memory() -> Option<u64> {
        let listed = std::fs::read_to_string("/proc/meminfo").ok()?;
        total_in(&listed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_says_the_whole_of_its_memory_on_the_memtotal_line() {
        let listed = "MemTotal:       32768000 kB\nMemFree:  1000 kB\nMemAvailable: 2000 kB\n";
        assert_eq!(total_in(listed), Some(32_768_000 * 1024));
        assert_eq!(total_in("MemFree: 1000 kB\n"), None);
        assert_eq!(total_in("MemTotal: lots\n"), None);
    }

    #[test]
    fn this_computer_has_some_memory_where_it_can_be_read() {
        if let Some(bytes) = total_memory() {
            assert!(bytes >= 256 << 20, "{bytes}");
        }
    }
}
