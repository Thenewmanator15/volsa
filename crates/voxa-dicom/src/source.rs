//! Where scan files come from: a folder, a zip read in place, or the folder beside a `DICOMDIR`.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use voxa_core::VoxaError;

/// Largest single entry that will be read into memory. Whole multi-frame series fit in one
/// file, so the limit is generous; it exists so a damaged or hostile archive cannot ask for
/// more memory than the machine has.
const MAX_ENTRY_BYTES: u64 = 4 * 1024 * 1024 * 1024;

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
fn read_limited(reader: &mut dyn Read, limit: u64) -> Result<Vec<u8>, VoxaError> {
    let mut bytes = Vec::new();
    let read = reader
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if read as u64 > limit {
        return Err(VoxaError::Archive(format!(
            "one of its files is larger than {} MB",
            limit / (1024 * 1024)
        )));
    }
    Ok(bytes)
}

/// Files under a folder on disk, or one file on its own.
struct FolderSource {
    /// (name relative to the root with `/`, path on disk), sorted by name.
    files: Vec<(String, PathBuf)>,
}

impl FolderSource {
    fn walk(root: &Path) -> Result<Self, VoxaError> {
        let mut files = Vec::new();
        collect_files(root, "", &mut files)?;
        files.sort();
        Ok(Self { files })
    }

    fn single(path: &Path) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            files: vec![(name, path.to_path_buf())],
        }
    }
}

/// Add every file under `folder` to `files`. Links to folders are not followed, so a loop of
/// links cannot trap the walk.
fn collect_files(
    folder: &Path,
    prefix: &str,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), VoxaError> {
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_files(&entry.path(), &format!("{name}/"), files)?;
        } else if kind.is_file() || (kind.is_symlink() && entry.path().is_file()) {
            files.push((name, entry.path()));
        }
    }
    Ok(())
}

impl SeriesSource for FolderSource {
    fn entries(&mut self) -> Box<dyn Iterator<Item = Result<Entry, VoxaError>> + '_> {
        Box::new(self.files.iter().map(|(name, path)| {
            let bytes = read_limited(&mut File::open(path)?, MAX_ENTRY_BYTES)?;
            Ok(Entry {
                name: name.clone(),
                bytes,
            })
        }))
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, VoxaError> {
        let found = self
            .files
            .binary_search_by(|(known, _)| known.as_str().cmp(name));
        let path = found
            .ok()
            .and_then(|at| self.files.get(at))
            .map(|(_, path)| path)
            .ok_or_else(|| VoxaError::MissingEntry(name.to_owned()))?;
        read_limited(&mut File::open(path)?, MAX_ENTRY_BYTES)
    }
}

/// The files inside a zip archive, read straight from the archive.
struct ZipSource {
    archive: zip::ZipArchive<BufReader<File>>,
    /// (name with `/`, position in the archive) of every file, sorted by name.
    files: Vec<(String, usize)>,
}

impl ZipSource {
    fn open(path: &Path) -> Result<Self, VoxaError> {
        let mut archive =
            zip::ZipArchive::new(BufReader::new(File::open(path)?)).map_err(archive_error)?;
        let mut files = Vec::new();
        for at in 0..archive.len() {
            let entry = archive.by_index_raw(at).map_err(archive_error)?;
            if entry.is_file() {
                let name = entry.name().map_err(archive_error)?;
                files.push((name.replace('\\', "/"), at));
            }
        }
        files.sort();
        Ok(Self { archive, files })
    }

    fn read_at(&mut self, at: usize) -> Result<Vec<u8>, VoxaError> {
        let mut entry = self.archive.by_index(at).map_err(archive_error)?;
        read_limited(&mut entry, MAX_ENTRY_BYTES)
    }
}

/// A zip library error as a message for the person using Voxa. Entry names are left out: they
/// can contain a patient's name.
fn archive_error(error: zip::result::ZipError) -> VoxaError {
    use zip::result::ZipError;
    match error {
        ZipError::Io(io) => VoxaError::Io(io),
        ZipError::UnsupportedArchive(what) => {
            VoxaError::Archive(format!("it uses a feature Voxa cannot read ({what})"))
        }
        ZipError::InvalidArchive(_) => VoxaError::Archive("it is damaged or incomplete".to_owned()),
        _ => VoxaError::Archive("it could not be opened".to_owned()),
    }
}

impl SeriesSource for ZipSource {
    fn entries(&mut self) -> Box<dyn Iterator<Item = Result<Entry, VoxaError>> + '_> {
        let files = self.files.clone();
        Box::new(files.into_iter().map(|(name, at)| {
            let bytes = self.read_at(at)?;
            Ok(Entry { name, bytes })
        }))
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, VoxaError> {
        let found = self
            .files
            .binary_search_by(|(known, _)| known.as_str().cmp(name));
        let at = found
            .ok()
            .and_then(|i| self.files.get(i))
            .map(|(_, at)| *at)
            .ok_or_else(|| VoxaError::MissingEntry(name.to_owned()))?;
        self.read_at(at)
    }
}

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
