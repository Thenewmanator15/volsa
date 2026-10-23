//! Where scan files come from: a folder, a zip read in place, or the folder beside a `DICOMDIR`.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use voxa_core::VoxaError;

mod archive_file;
mod folder;
mod zipped;

use folder::FolderSource;
use zipped::ZipSource;

/// Largest single entry that will be read into memory. Whole multi-frame series fit in one
/// file, so the limit is generous; it exists so a damaged or hostile archive cannot ask for
/// more memory than the machine has.
pub(crate) const MAX_ENTRY_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// One file from a source: its name relative to the source's root, with `/` between folders,
/// and its contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Name relative to the source's root, with `/` between folders.
    pub name: String,
    /// The file's contents.
    pub bytes: Vec<u8>,
}

/// Somewhere scan files can be read from.
pub trait SeriesSource: Send {
    /// Every file in the source, in name order, read one at a time.
    fn entries(&mut self) -> Box<dyn Iterator<Item = Result<Entry, VoxaError>> + '_>;

    /// One file again by the name [`SeriesSource::entries`] gave it, for decoding later.
    ///
    /// # Errors
    /// [`VoxaError::MissingEntry`] when the source has no such file; an I/O or archive error
    /// when it cannot be read.
    fn read(&mut self, name: &str) -> Result<Vec<u8>, VoxaError>;

    /// Every file in the source, in name order, handed to `visit` with its name and a reader
    /// of its contents, so that no more of a file is read than `visit` reads. Indexing reads
    /// each file only as far as its header goes.
    ///
    /// The default reads each file whole through [`SeriesSource::entries`]; a folder and a zip
    /// read only what `visit` asks for.
    ///
    /// # Errors
    /// The first error met opening or reading a file, or given by `visit`. No file after it
    /// is visited. A file larger than a source will read whole is refused before it is visited.
    fn visit_files(
        &mut self,
        visit: &mut dyn FnMut(&str, &mut dyn Read) -> Result<(), VoxaError>,
    ) -> Result<(), VoxaError> {
        for entry in self.entries() {
            let entry = entry?;
            visit(&entry.name, &mut entry.bytes.as_slice())?;
        }
        Ok(())
    }

    /// Another reader of the same files, which reads them without waiting for this one, so
    /// that each thread decoding slices can read with a reader of its own. `None` when the
    /// source cannot give one and its reads must take turns.
    ///
    /// Folders and zips give one: a zip's readers share what was read of its directory.
    fn another_reader(&self) -> Option<Box<dyn SeriesSource>> {
        None
    }
}

/// Open a folder, a zip file, a `DICOMDIR` (the folder beside it is read) or a single file.
///
/// A zip is read in place: nothing is extracted to disk.
///
/// # Errors
/// [`VoxaError::Io`] when the path does not exist or cannot be read, and
/// [`VoxaError::Archive`] when a file with a zip signature is not a readable archive.
pub fn open_source(path: &Path) -> Result<Box<dyn SeriesSource>, VoxaError> {
    let meta = std::fs::metadata(path)?;
    if meta.is_dir() {
        return Ok(Box::new(FolderSource::walk(path)?));
    }
    if path
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("DICOMDIR"))
    {
        let folder = path.parent().unwrap_or_else(|| Path::new("."));
        return Ok(Box::new(FolderSource::walk(folder)?));
    }
    if has_zip_signature(path)? {
        return Ok(Box::new(ZipSource::open(path)?));
    }
    Ok(Box::new(FolderSource::single(path)))
}

fn has_zip_signature(path: &Path) -> Result<bool, VoxaError> {
    let mut start = [0u8; 4];
    let read = File::open(path)?.read(&mut start)?;
    Ok(read == 4 && start == *b"PK\x03\x04")
}

/// Read everything from `reader`, refusing more than `limit` bytes.
pub(crate) fn read_limited(reader: &mut dyn Read, limit: u64) -> Result<Vec<u8>, VoxaError> {
    let mut bytes = Vec::new();
    let read = reader
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    within_limit(read as u64, limit)?;
    Ok(bytes)
}

/// Refuse a file of `len` bytes when that is more than `limit`.
pub(crate) fn within_limit(len: u64, limit: u64) -> Result<(), VoxaError> {
    if len > limit {
        return Err(too_large(limit));
    }
    Ok(())
}

/// Why a file larger than `limit` bytes is not read.
pub(crate) fn too_large(limit: u64) -> VoxaError {
    VoxaError::Archive(format!(
        "one of its files is larger than {} MB",
        limit / (1024 * 1024)
    ))
}

#[cfg(test)]
mod reader_tests;
#[cfg(test)]
mod visit_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    fn names_and_bytes(source: &mut dyn SeriesSource) -> Vec<(String, Vec<u8>)> {
        source
            .entries()
            .map(|entry| entry.map(|e| (e.name, e.bytes)))
            .collect::<Result<Vec<_>, _>>()
            .expect("every entry reads")
    }

    fn write(root: &Path, relative: &str, bytes: &[u8]) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("has a parent")).expect("folder is made");
        fs::write(path, bytes).expect("file is written");
    }

    fn listing(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(folder)
            .expect("folder lists")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn zip_of(path: &Path, files: &[(&str, &[u8])]) {
        let mut writer = zip::ZipWriter::new(fs::File::create(path).expect("zip is created"));
        let options = zip::write::SimpleFileOptions::default();
        writer
            .add_directory("IMAGES/", options)
            .expect("directory entry");
        for (name, bytes) in files {
            writer.start_file(*name, options).expect("entry starts");
            writer.write_all(bytes).expect("entry is written");
        }
        writer.finish().expect("zip is finished");
    }

    #[test]
    fn a_folder_yields_every_file_once_with_slash_names() {
        let dir = tempfile::tempdir().expect("temp dir");
        write(dir.path(), "b.dcm", b"bee");
        write(dir.path(), "IMAGES/S1/I0002", b"two");
        write(dir.path(), "IMAGES/S1/I0001", b"one");
        let mut source = open_source(dir.path()).expect("folder opens");
        assert_eq!(
            names_and_bytes(source.as_mut()),
            vec![
                ("IMAGES/S1/I0001".to_owned(), b"one".to_vec()),
                ("IMAGES/S1/I0002".to_owned(), b"two".to_vec()),
                ("b.dcm".to_owned(), b"bee".to_vec()),
            ]
        );
    }

    #[test]
    fn a_zip_is_read_in_place_without_extracting() {
        let dir = tempfile::tempdir().expect("temp dir");
        let zip_path = dir.path().join("export.zip");
        zip_of(
            &zip_path,
            &[
                ("IMAGES/S1/I0001", b"one"),
                ("IMAGES/S1/I0002", b"two"),
                ("b.dcm", b"bee"),
            ],
        );
        let before = listing(dir.path());
        let mut source = open_source(&zip_path).expect("zip opens");
        assert_eq!(
            names_and_bytes(source.as_mut()),
            vec![
                ("IMAGES/S1/I0001".to_owned(), b"one".to_vec()),
                ("IMAGES/S1/I0002".to_owned(), b"two".to_vec()),
                ("b.dcm".to_owned(), b"bee".to_vec()),
            ]
        );
        assert_eq!(
            listing(dir.path()),
            before,
            "nothing was written beside the zip"
        );
        assert_eq!(before, vec!["export.zip".to_owned()]);
    }

    #[test]
    fn a_dicomdir_path_yields_the_files_beside_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        write(dir.path(), "DICOMDIR", b"index");
        write(dir.path(), "IMAGES/I0001", b"one");
        let mut source = open_source(&dir.path().join("DICOMDIR")).expect("DICOMDIR opens");
        let names: Vec<String> = names_and_bytes(source.as_mut())
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(
            names,
            vec!["DICOMDIR".to_owned(), "IMAGES/I0001".to_owned()]
        );
    }

    #[test]
    fn a_single_file_that_is_not_a_zip_is_a_source_of_one() {
        let dir = tempfile::tempdir().expect("temp dir");
        write(dir.path(), "slice.dcm", b"not a zip");
        write(dir.path(), "other.dcm", b"ignored");
        let mut source = open_source(&dir.path().join("slice.dcm")).expect("file opens");
        assert_eq!(
            names_and_bytes(source.as_mut()),
            vec![("slice.dcm".to_owned(), b"not a zip".to_vec())]
        );
    }

    #[test]
    fn a_missing_path_is_an_io_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let result = open_source(&dir.path().join("nothing-here"));
        assert!(matches!(result, Err(VoxaError::Io(_))));
    }

    #[test]
    fn an_empty_folder_has_no_entries() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut source = open_source(dir.path()).expect("folder opens");
        assert_eq!(names_and_bytes(source.as_mut()), Vec::new());
    }

    #[test]
    fn an_entry_can_be_read_again_by_name_from_either_kind_of_source() {
        let dir = tempfile::tempdir().expect("temp dir");
        write(dir.path(), "study/IMAGES/I0001", b"one");
        let zip_path = dir.path().join("export.zip");
        zip_of(&zip_path, &[("IMAGES/I0001", b"one")]);
        let mut folder = open_source(&dir.path().join("study")).expect("folder opens");
        let mut zipped = open_source(&zip_path).expect("zip opens");
        assert_eq!(folder.read("IMAGES/I0001").expect("reads"), b"one");
        assert_eq!(zipped.read("IMAGES/I0001").expect("reads"), b"one");
        assert!(
            matches!(folder.read("IMAGES/I9999"), Err(VoxaError::MissingEntry(name)) if name == "IMAGES/I9999")
        );
        assert!(matches!(
            zipped.read("IMAGES/I9999"),
            Err(VoxaError::MissingEntry(_))
        ));
    }

    #[test]
    fn a_zip_signature_on_a_broken_archive_is_an_archive_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        write(
            dir.path(),
            "broken.zip",
            b"PK\x03\x04 this is not a real archive",
        );
        let result = open_source(&dir.path().join("broken.zip"));
        assert!(matches!(result, Err(VoxaError::Archive(_))));
    }

    #[test]
    fn reading_stops_at_the_size_limit() {
        let data = vec![7u8; 100];
        assert_eq!(read_limited(&mut data.as_slice(), 100).expect("fits"), data);
        assert!(matches!(
            read_limited(&mut data.as_slice(), 99),
            Err(VoxaError::Archive(_))
        ));
    }
}
