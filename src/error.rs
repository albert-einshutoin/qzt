use std::fmt;

/// Top-level error type for QZT operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QztError {
    InvalidMagic,
    UnsupportedVersion,
    InvalidHeader,
    InvalidFooterTrailer,
    InvalidFooterPayload,
    NonCanonicalCbor,
    DuplicateCborKey,
    FooterChecksumMismatch,
    FinalFileSizeMismatch,
    ContainerIdMismatch,
    MetadataChecksumMismatch,
    MetadataInvalid,
    VersionMismatch,
    NewlineModeMismatch,
    IndexRootChecksumMismatch,
    MissingRequiredBlock,
    UnknownRequiredBlock,
    InvalidFlags,
    ChunkTableChecksumMismatch,
    ChunkTableInvalid,
    ChunkCountMismatch,
    ChunkSizeMismatch,
    PhysicalRangeOutOfBounds,
    LogicalRangeOutOfBounds,
    InvalidUtf8,
    InvalidUtf8Boundary,
    LineOutOfRange,
    MissingDictionary,
    DictionaryChecksumMismatch,
    CompressedChunkChecksumMismatch,
    UncompressedChunkChecksumMismatch,
    ZstdEncodeError,
    ZstdDecodeError,
    ContainerCorrupt,
    ResourceLimitExceeded,
    RangeOverlap,
    UnexpectedEof,
    WriterAlreadyFinished,
    VerifiedChecksumMismatch,

    /// Placeholder used until format-specific errors are introduced.
    NotImplemented(&'static str),
}

impl fmt::Display for QztError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented(feature) => write!(f, "{feature} is not implemented yet"),
            error => write!(f, "{error:?}"),
        }
    }
}

impl std::error::Error for QztError {}

/// Result alias used by public QZT APIs.
pub type Result<T> = std::result::Result<T, QztError>;
