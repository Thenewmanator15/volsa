//! Volsa's own file for the slabs of a volume past the memory budget. It lives in the `cache`
//! folder of Volsa's folder on this computer, is named `spill-` and a random number, is opened by
//! Volsa alone (on Windows no other program can open it while Volsa has it), holds the slabs'
//! numbers and nothing else (no name, no tag, nothing to say whose scan it is), and is deleted
//! when its volume gives way, when Volsa closes, and at the next start after a stop.

use std::fs::{File, OpenOptions};
use std::hash::{BuildHasher, RandomState};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use volsa_core::SlabStore;

/// The folder, within Volsa's folder on this computer, the file is made in.
const FOLDER: &str = "cache";
/// How every spill file's name starts and ends.
const STARTS: &str = "spill-";
const ENDS: &str = ".volsa";

/// A spill file for one volume, its slabs each at their own place.
pub struct SpillFile {
    file: Mutex<Option<File>>,
    path: PathBuf,
    /// The bytes from one slab's place to the next: those of a whole slab.
    slab_bytes: u64,
    /// What is done when a slab has been brought back, and memory is used again.
    brought_back: Option<Box<dyn Fn() + Send + Sync>>,
}

impl SpillFile {
    /// A new, empty spill file in `local`'s cache folder, for slabs of `slab_bytes`.
    ///
    /// # Errors
    ///
    /// When the folder or the file cannot be made.
    pub fn create(local: &Path, slab_bytes: usize) -> io::Result<Self> {
        let folder = local.join(FOLDER);
        std::fs::create_dir_all(&folder)?;
        let number = RandomState::new().hash_one(std::process::id());
        let path = folder.join(format!("{STARTS}{number:016x}{ENDS}"));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // Opened by Volsa alone: no other program may read, write or delete it meanwhile.
            options.share_mode(0);
        }
        let file = options.open(&path)?;
        Ok(Self {
            file: Mutex::new(Some(file)),
            path,
            slab_bytes: u64::try_from(slab_bytes).unwrap_or(u64::MAX),
            brought_back: None,
        })
    }

    /// Have `call` called each time a slab has been brought back into memory.
    #[must_use]
    pub fn when_brought_back(mut self, call: impl Fn() + Send + Sync + 'static) -> Self {
        self.brought_back = Some(Box::new(call));
        self
    }

    /// Where byte `at` of slab `slab` is kept.
    fn place(&self, slab: usize, at: usize) -> u64 {
        let start =
            u64::try_from(slab).map_or(u64::MAX, |slab| slab.saturating_mul(self.slab_bytes));
        start.saturating_add(u64::try_from(at).unwrap_or(u64::MAX))
    }

    /// The file's length, in bytes.
    #[cfg(test)]
    fn len(&self) -> io::Result<u64> {
        let file = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(file.as_ref().ok_or_else(closed)?.metadata()?.len())
    }

    #[cfg(test)]
    fn path(&self) -> &Path {
        &self.path
    }
}

impl SlabStore for SpillFile {
    fn put_away(&self, slab: usize, at: usize, bytes: &[u8]) -> io::Result<()> {
        let mut file = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        let file = file.as_mut().ok_or_else(closed)?;
        file.seek(SeekFrom::Start(self.place(slab, at)))?;
        file.write_all(bytes)
    }

    fn bring_back(&self, slab: usize, at: usize, into: &mut [u8]) -> io::Result<()> {
        let mut file = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        let read = file.as_mut().ok_or_else(closed).and_then(|file| {
            file.seek(SeekFrom::Start(self.place(slab, at)))?;
            file.read_exact(into)
        });
        drop(file);
        if let Err(why) = &read {
            // Said by kind alone: the file's name and what it holds are nobody's business.
            tracing::warn!("a slab could not be read back from disk: {}", why.kind());
        }
        read
    }

    fn brought_back(&self) {
        if let Some(call) = &self.brought_back {
            call();
        }
    }
}

impl Drop for SpillFile {
    /// The file goes with its volume.
    fn drop(&mut self) {
        let file = self.file.get_mut().unwrap_or_else(PoisonError::into_inner);
        drop(file.take());
        let _ = std::fs::remove_file(&self.path);
    }
}

fn closed() -> io::Error {
    io::Error::other("the spill file is closed")
}

/// Delete every spill file left in `local`'s cache folder by a Volsa that stopped without
/// deleting its own. Gives how many went. One still open by a Volsa running is left.
pub fn clear_leftovers(local: &Path) -> usize {
    let Ok(listed) = std::fs::read_dir(local.join(FOLDER)) else {
        return 0;
    };
    listed
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with(STARTS) && name.ends_with(ENDS)
        })
        .filter(|entry| std::fs::remove_file(entry.path()).is_ok())
        .count()
}

#[cfg(test)]
mod tests;
