//! The error type shared by Volsa's library crates.

/// Something went wrong reading or decoding a scan. Each message is written for the person
/// using Volsa and never contains patient details.
#[derive(Debug, thiserror::Error)]
pub enum VolsaError {
    /// A file or folder could not be read.
    #[error("The file could not be read: {0}.")]
    Io(#[from] std::io::Error),
    /// A zip archive could not be opened or one of its entries could not be read.
    #[error("The zip file could not be read: {0}.")]
    Archive(String),
    /// The bytes are not a DICOM file.
    #[error("This is not a DICOM file.")]
    NotDicom,
    /// The pixel data could not be turned into an image.
    #[error("The image could not be decoded: {0}.")]
    Decode(String),
    /// The pixel data uses a compression Volsa cannot read; its name is given.
    #[error("The images are compressed in a way Volsa cannot read yet ({0}).")]
    UnsupportedTransferSyntax(String),
    /// The source holds no images.
    #[error("No images were found.")]
    EmptySource,
    /// An entry asked for by name is not in the source.
    #[error("The export no longer contains {0}.")]
    MissingEntry(String),
}

impl VolsaError {
    /// What kind of failure this is, in a few words that say nothing of the file it happened
    /// on: for a log, which must not hold a file's name or anything read from one.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Io(_) => "a file could not be read",
            Self::Archive(_) => "a zip file could not be read",
            Self::NotDicom => "not a DICOM file",
            Self::Decode(_) => "an image could not be decoded",
            Self::UnsupportedTransferSyntax(_) => "a compression Volsa cannot read",
            Self::EmptySource => "no images were found",
            Self::MissingEntry(_) => "a file is no longer in the export",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_of_a_failure_says_nothing_of_the_file() {
        let named = "Quillfeather.dcm".to_owned();
        let failures = [
            VolsaError::Archive(named.clone()),
            VolsaError::Decode(named.clone()),
            VolsaError::UnsupportedTransferSyntax(named.clone()),
            VolsaError::MissingEntry(named),
        ];
        for failure in failures {
            assert!(failure.to_string().contains("Quillfeather"));
            assert!(
                !failure.kind().contains("Quillfeather"),
                "{}",
                failure.kind()
            );
        }
        assert_eq!(VolsaError::NotDicom.kind(), "not a DICOM file");
    }

    #[test]
    fn messages_are_plain_words() {
        assert_eq!(
            VolsaError::NotDicom.to_string(),
            "This is not a DICOM file."
        );
        assert_eq!(VolsaError::EmptySource.to_string(), "No images were found.");
        assert_eq!(
            VolsaError::UnsupportedTransferSyntax("JPEG XL".to_owned()).to_string(),
            "The images are compressed in a way Volsa cannot read yet (JPEG XL)."
        );
        assert_eq!(
            VolsaError::Decode("pixel data is shorter than the image size".to_owned()).to_string(),
            "The image could not be decoded: pixel data is shorter than the image size."
        );
    }

    #[test]
    fn an_io_error_converts_and_keeps_its_cause() {
        let missing = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let error = VolsaError::from(missing);
        assert!(matches!(error, VolsaError::Io(_)));
        assert!(error.to_string().starts_with("The file could not be read"));
        assert!(std::error::Error::source(&error).is_some());
    }
}
