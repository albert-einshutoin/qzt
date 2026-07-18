use std::fmt;

/// Top-level error type for QZT operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum QztError {
    /// The fixed header does not start with the QZT magic bytes.
    InvalidMagic,
    /// The container or sidecar version is not supported.
    UnsupportedVersion,
    /// The fixed QZT header is malformed.
    InvalidHeader,
    /// The fixed footer trailer is malformed.
    InvalidFooterTrailer,
    /// The footer CBOR payload is malformed.
    InvalidFooterPayload,
    /// CBOR input is not encoded in QZT's deterministic canonical form.
    NonCanonicalCbor,
    /// A CBOR map repeats a key.
    DuplicateCborKey,
    /// The footer payload does not match its declared checksum.
    FooterChecksumMismatch,
    /// The physical file size does not match the footer declaration.
    FinalFileSizeMismatch,
    /// Two structures that must identify the same container disagree.
    ContainerIdMismatch,
    /// The metadata block does not match its declared checksum.
    MetadataChecksumMismatch,
    /// Metadata violates the QZT schema or an internal invariant.
    MetadataInvalid,
    /// A Document Index contains more than one entry with the same identifier.
    DuplicateDocumentId,
    /// Related container blocks declare incompatible format versions.
    VersionMismatch,
    /// Metadata and chunk data disagree about newline handling.
    NewlineModeMismatch,
    /// The index-root block does not match its declared checksum.
    IndexRootChecksumMismatch,
    /// A block required by the selected profile is absent.
    MissingRequiredBlock,
    /// A block with an unknown type is marked as required.
    UnknownRequiredBlock,
    /// The requested document identifier is absent from the Document Index.
    DocumentNotFound,
    /// Reserved flag bits are set or an invalid flag combination was found.
    InvalidFlags,
    /// The chunk-table block does not match its declared checksum.
    ChunkTableChecksumMismatch,
    /// The chunk table violates ordering, size, or range invariants.
    ChunkTableInvalid,
    /// Declared and observed chunk counts differ.
    ChunkCountMismatch,
    /// Declared and observed chunk sizes differ.
    ChunkSizeMismatch,
    /// A physical file range lies outside the available bytes.
    PhysicalRangeOutOfBounds,
    /// A logical source range lies outside the original content.
    LogicalRangeOutOfBounds,
    /// Content required to be UTF-8 is not valid UTF-8.
    InvalidUtf8,
    /// A requested text range splits a UTF-8 code point.
    InvalidUtf8Boundary,
    /// A requested line number or line range is unavailable.
    LineOutOfRange,
    /// A chunk references a dictionary that is not present.
    MissingDictionary,
    /// An embedded dictionary does not match its declared checksum.
    DictionaryChecksumMismatch,
    /// Compressed chunk bytes do not match their declared checksum.
    CompressedChunkChecksumMismatch,
    /// Decoded chunk bytes do not match their declared checksum.
    UncompressedChunkChecksumMismatch,
    /// zstd failed while encoding a chunk.
    ZstdEncodeError,
    /// zstd failed while decoding a chunk.
    ZstdDecodeError,
    /// A container or sidecar violates a cross-field integrity invariant.
    ContainerCorrupt,
    /// A configured limit, representable size, or checked range arithmetic was exceeded.
    ResourceLimitExceeded,
    /// Declared physical or logical ranges overlap unexpectedly.
    RangeOverlap,
    /// Input ended before the declared structure was complete.
    UnexpectedEof,
    /// A streaming writer operation was attempted after finalization or a prior write failure.
    WriterAlreadyFinished,
    /// Restored bytes do not match a caller-supplied expected checksum.
    VerifiedChecksumMismatch,
    /// Benchmark output failed an internal consistency check.
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
            Self::DuplicateDocumentId => "document index contains a duplicate document id",
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
