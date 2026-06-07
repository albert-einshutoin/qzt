use crate::error::{QztError, Result};

/// Placeholder writer entry point reserved for later phases.
pub struct QztWriter;

impl QztWriter {
    /// Creates a QZT writer.
    pub fn new() -> Result<Self> {
        Err(QztError::NotImplemented("QztWriter::new"))
    }
}
