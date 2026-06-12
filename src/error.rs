use std::fmt;

/// Top-level error type for QZT operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
    DocumentNotFound,
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
    BenchmarkMetricsMismatch,

    /// OS-level I/O error (file not found, permission denied, write failure, etc.).
    Io(std::io::ErrorKind),
    /// The requested index mode is not supported by this implementation.
    UnsupportedIndexMode(&'static str),
}

impl fmt::Display for QztError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidMagic => "invalid magic bytes: not a QZT container",
            Self::UnsupportedVersion => "unsupported QZT format version",
            Self::InvalidHeader => "fixed header is malformed",
            Self::InvalidFooterTrailer => "fixed footer trailer is malformed",
            Self::InvalidFooterPayload => "footer payload is malformed",
            Self::NonCanonicalCbor => "CBOR is not in the deterministic canonical form",
            Self::DuplicateCborKey => "CBOR map contains duplicate keys",
            Self::FooterChecksumMismatch => "footer payload checksum mismatch",
            Self::FinalFileSizeMismatch => "final file size does not match the footer",
            Self::ContainerIdMismatch => "container id mismatch between header and footer",
            Self::MetadataChecksumMismatch => "metadata block checksum mismatch",
            Self::MetadataInvalid => "metadata block is invalid",
            Self::VersionMismatch => "version mismatch between blocks",
            Self::NewlineModeMismatch => "newline mode mismatch between metadata and content",
            Self::IndexRootChecksumMismatch => "index root checksum mismatch",
            Self::MissingRequiredBlock => "a required block is missing",
            Self::UnknownRequiredBlock => "an unknown block is marked as required",
            Self::DocumentNotFound => "document id not found in the document index",
            Self::InvalidFlags => "reserved flags contain unexpected bits",
            Self::ChunkTableChecksumMismatch => "chunk table checksum mismatch",
            Self::ChunkTableInvalid => "chunk table is invalid",
            Self::ChunkCountMismatch => "chunk count mismatch",
            Self::ChunkSizeMismatch => "chunk size mismatch",
            Self::PhysicalRangeOutOfBounds => "physical byte range is out of bounds",
            Self::LogicalRangeOutOfBounds => "logical byte range is out of bounds",
            Self::InvalidUtf8 => "content is not valid UTF-8",
            Self::InvalidUtf8Boundary => "range does not start or end on a UTF-8 boundary",
            Self::LineOutOfRange => "line number is out of range",
            Self::MissingDictionary => "a chunk references a missing dictionary",
            Self::DictionaryChecksumMismatch => "dictionary checksum mismatch",
            Self::CompressedChunkChecksumMismatch => "compressed chunk checksum mismatch",
            Self::UncompressedChunkChecksumMismatch => "uncompressed chunk checksum mismatch",
            Self::ZstdEncodeError => "zstd compression failed",
            Self::ZstdDecodeError => "zstd decompression failed",
            Self::ContainerCorrupt => "container is corrupt",
            Self::ResourceLimitExceeded => "a resource limit was exceeded",
            Self::RangeOverlap => "physical ranges overlap",
            Self::UnexpectedEof => "unexpected end of input",
            Self::WriterAlreadyFinished => "writer has already been finished",
            Self::VerifiedChecksumMismatch => "verified content checksum mismatch",
            Self::BenchmarkMetricsMismatch => "benchmark metrics mismatch",
            Self::Io(kind) => return write!(f, "I/O error: {kind}"),
            Self::UnsupportedIndexMode(mode) => {
                return write!(f, "index mode {mode} is not supported");
            }
        };
        f.write_str(message)
    }
}

impl std::error::Error for QztError {}

/// Result alias used by public QZT APIs.
pub type Result<T> = std::result::Result<T, QztError>;
