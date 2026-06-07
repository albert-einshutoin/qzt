use std::fmt;

/// Top-level error type for QZT operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QztError {
    /// Placeholder used until format-specific errors are introduced.
    NotImplemented(&'static str),
}

impl fmt::Display for QztError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented(feature) => write!(f, "{feature} is not implemented yet"),
        }
    }
}

impl std::error::Error for QztError {}

/// Result alias used by public QZT APIs.
pub type Result<T> = std::result::Result<T, QztError>;
