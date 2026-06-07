use crate::error::{QztError, Result};

/// Placeholder reader entry point reserved for later phases.
pub struct QztReader;

impl QztReader {
    /// Opens a QZT container.
    pub fn open() -> Result<Self> {
        Err(QztError::NotImplemented("QztReader::open"))
    }
}
