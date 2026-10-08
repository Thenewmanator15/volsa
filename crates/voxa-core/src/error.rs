//! The error type shared by Voxa's library crates.

/// Something went wrong reading or decoding a scan. Each message is written for the person
/// using Voxa and never contains patient details.
#[derive(Debug, thiserror::Error)]
pub enum VoxaError {
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
    /// The pixel data uses a compression Voxa cannot read; its name is given.
    #[error("The images are compressed in a way Voxa cannot read yet ({0}).")]
    UnsupportedTransferSyntax(String),
    /// The source holds no images.
    #[error("No images were found.")]
    EmptySource,
    /// An entry asked for by name is not in the source.
    #[error("The export no longer contains {0}.")]
    MissingEntry(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_plain_words() {
        assert_eq!(VoxaError::NotDicom.to_string(), "This is not a DICOM file.");
        assert_eq!(VoxaError::EmptySource.to_string(), "No images were found.");
        assert_eq!(
            VoxaError::UnsupportedTransferSyntax("JPEG XL".to_owned()).to_string(),
            "The images are compressed in a way Voxa cannot read yet (JPEG XL)."
        );
        assert_eq!(
            VoxaError::Decode("pixel data is shorter than the image size".to_owned()).to_string(),
            "The image could not be decoded: pixel data is shorter than the image size."
        );
    }

    #[test]
    fn an_io_error_converts_and_keeps_its_cause() {
        let missing = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let error = VoxaError::from(missing);
        assert!(matches!(error, VoxaError::Io(_)));
        assert!(error.to_string().starts_with("The file could not be read"));
        assert!(std::error::Error::source(&error).is_some());
    }
}
