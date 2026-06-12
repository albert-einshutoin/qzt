use crate::cbor::{encode_deterministic, validate_deterministic, CborValue};
use crate::error::{QztError, Result};
use crate::primitives::usize_to_u64;
use std::collections::BTreeSet;

const SCHEMA_FOOTER: &str = "qzt.footer.v1";
const SCHEMA_METADATA: &str = "qzt.metadata.v1";
const SCHEMA_INDEX_ROOT: &str = "qzt.index-root.v1";
const SCHEMA_DICTIONARY: &str = "qzt.dictionary.v1";
const SCHEMA_DOCUMENT_INDEX: &str = "qzt.document-index.v1";
/// BLAKE3 algorithm identifier used in all QZT checksum fields.
pub(crate) const CHECKSUM_ALGORITHM_BLAKE3: &str = "blake3";
const CHUNK_TABLE_TYPE: &str = "chunk_table";
const CHUNK_TABLE_CODEC: &str = "qzt-ctbl-fixed-v1";
const DICTIONARY_TYPE: &str = "dictionary";
const DICTIONARY_CODEC: &str = "qzt-dict-cbor-v1";
const DENSE_LINE_INDEX_TYPE: &str = "dense_line_index";
const DENSE_LINE_INDEX_CODEC: &str = "qzt-line-delta-varint-v1";
const DOCUMENT_INDEX_TYPE: &str = "document_index";
const DOCUMENT_INDEX_CODEC: &str = "qzt-doc-index-cbor-v1";

/// BLAKE3 checksum value used by QZT Core structures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksum {
    pub algorithm: String,
    pub value: [u8; 32],
}

impl Checksum {
    /// Computes a BLAKE3 checksum over the given bytes.
    #[must_use]
    pub fn blake3(bytes: &[u8]) -> Self {
        Self {
            algorithm: CHECKSUM_ALGORITHM_BLAKE3.to_owned(),
            value: *blake3::hash(bytes).as_bytes(),
        }
    }

    /// Finalizes a streaming BLAKE3 hasher into a [`Checksum`].
    #[must_use]
    pub(crate) fn from_hasher(hasher: &blake3::Hasher) -> Self {
        Self {
            algorithm: CHECKSUM_ALGORITHM_BLAKE3.to_owned(),
            value: *hasher.finalize().as_bytes(),
        }
    }

    /// Constructs a BLAKE3 [`Checksum`] from pre-computed raw hash bytes.
    ///
    /// The caller is responsible for ensuring the bytes were produced by BLAKE3.
    #[must_use]
    pub(crate) fn from_raw_bytes(value: [u8; 32]) -> Self {
        Self {
            algorithm: CHECKSUM_ALGORITHM_BLAKE3.to_owned(),
            value,
        }
    }
}

/// Offset, size, and checksum for a referenced CBOR or binary block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockRef {
    pub offset: u64,
    pub size: u64,
    pub checksum: Checksum,
}

/// Footer Payload logical model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FooterPayload {
    pub container_id: [u8; 16],
    pub index_root: BlockRef,
    pub metadata: BlockRef,
    pub final_file_size: u64,
    pub footer_flags: u64,
    pub container_checksum: Option<Checksum>,
}

impl FooterPayload {
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut entries = vec![
            text_pair("schema", CborValue::Text(SCHEMA_FOOTER.to_owned())),
            text_pair("format_version", version_value()),
            text_pair("container_id", CborValue::Bytes(self.container_id.to_vec())),
            text_pair("index_root", block_ref_value(&self.index_root)),
            text_pair("metadata", block_ref_value(&self.metadata)),
            text_pair("final_file_size", u64_value(self.final_file_size)),
            text_pair("footer_flags", u64_value(self.footer_flags)),
        ];
        if let Some(checksum) = &self.container_checksum {
            entries.push(text_pair("container_checksum", checksum_value(checksum)));
        }
        encode_deterministic(&CborValue::Map(entries))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let value = validate_deterministic(bytes)?;
        let map = as_map(&value, QztError::InvalidFooterPayload)?;
        reject_unknown_keys(
            map,
            &[
                "schema",
                "format_version",
                "container_id",
                "index_root",
                "metadata",
                "final_file_size",
                "footer_flags",
                "created_at_unix_ms",
                "writer",
                "container_checksum",
            ],
            QztError::InvalidFooterPayload,
        )?;
        expect_text_field(map, "schema", SCHEMA_FOOTER, QztError::InvalidFooterPayload)?;
        expect_version_field(map, QztError::InvalidFooterPayload)?;

        let footer_flags = required_u64(map, "footer_flags", QztError::InvalidFooterPayload)?;
        if footer_flags != 0 {
            return Err(QztError::InvalidFlags);
        }

        Ok(Self {
            container_id: required_bstr16(map, "container_id", QztError::InvalidFooterPayload)?,
            index_root: required_block_ref(map, "index_root", QztError::InvalidFooterPayload)?,
            metadata: required_block_ref(map, "metadata", QztError::InvalidFooterPayload)?,
            final_file_size: required_u64(map, "final_file_size", QztError::InvalidFooterPayload)?,
            footer_flags,
            container_checksum: optional_checksum(
                map,
                "container_checksum",
                QztError::InvalidFooterPayload,
            )?,
        })
    }
}

/// Metadata fields needed for Core structural verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub container_id: [u8; 16],
    pub original_size: u64,
    pub original_checksum: Checksum,
    pub newline_mode: String,
    pub line_count: u64,
    pub zstd_level: i32,
    pub target_chunk_size: u64,
    pub max_chunk_size: u64,
    pub dictionary_mode: String,
    pub profile: String,
    pub dense_line_index: bool,
    pub document_index: bool,
}

/// Metadata writer options that are not derived from original bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataOptions<'a> {
    pub zstd_level: i32,
    pub target_chunk_size: u64,
    pub max_chunk_size: u64,
    pub dictionary_mode: &'a str,
    pub profile: &'a str,
    pub dense_line_index: bool,
    pub document_index: bool,
}

impl Default for MetadataOptions<'_> {
    fn default() -> Self {
        Self {
            zstd_level: 0,
            target_chunk_size: 4 * 1024 * 1024,
            max_chunk_size: 16 * 1024 * 1024,
            dictionary_mode: "none",
            profile: "core",
            dense_line_index: false,
            document_index: false,
        }
    }
}

impl Metadata {
    #[must_use]
    pub fn empty(container_id: [u8; 16]) -> Self {
        Self::for_source(container_id, 0, Checksum::blake3(&[]), "none", 0)
    }

    #[must_use]
    pub fn for_source(
        container_id: [u8; 16],
        original_size: u64,
        original_checksum: Checksum,
        newline_mode: &str,
        line_count: u64,
    ) -> Self {
        Self::for_source_with_dictionary_mode(
            container_id,
            original_size,
            original_checksum,
            newline_mode,
            line_count,
            "none",
        )
    }

    #[must_use]
    pub fn for_source_with_dictionary_mode(
        container_id: [u8; 16],
        original_size: u64,
        original_checksum: Checksum,
        newline_mode: &str,
        line_count: u64,
        dictionary_mode: &str,
    ) -> Self {
        let options = MetadataOptions {
            dictionary_mode,
            ..MetadataOptions::default()
        };
        Self::for_source_with_options(
            container_id,
            original_size,
            original_checksum,
            newline_mode,
            line_count,
            options,
        )
    }

    #[must_use]
    pub fn for_source_with_options(
        container_id: [u8; 16],
        original_size: u64,
        original_checksum: Checksum,
        newline_mode: &str,
        line_count: u64,
        options: MetadataOptions<'_>,
    ) -> Self {
        Self {
            container_id,
            original_size,
            original_checksum,
            newline_mode: newline_mode.to_owned(),
            line_count,
            zstd_level: options.zstd_level,
            target_chunk_size: options.target_chunk_size,
            max_chunk_size: options.max_chunk_size,
            dictionary_mode: options.dictionary_mode.to_owned(),
            profile: options.profile.to_owned(),
            dense_line_index: options.dense_line_index,
            document_index: options.document_index,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        encode_deterministic(&CborValue::Map(vec![
            text_pair("schema", CborValue::Text(SCHEMA_METADATA.to_owned())),
            text_pair("format", CborValue::Text("qzt".to_owned())),
            text_pair("format_version", version_value()),
            text_pair("container_id", CborValue::Bytes(self.container_id.to_vec())),
            text_pair(
                "identity",
                CborValue::Map(vec![
                    text_pair("name", CborValue::Null),
                    text_pair("profile", CborValue::Text(self.profile.clone())),
                    text_pair("created_by", CborValue::Text("qzt".to_owned())),
                    text_pair("created_at_unix_ms", CborValue::Null),
                ]),
            ),
            text_pair(
                "source",
                CborValue::Map(vec![
                    text_pair("media_type", CborValue::Text("text".to_owned())),
                    text_pair("encoding", CborValue::Text("utf-8".to_owned())),
                    text_pair("original_size", u64_value(self.original_size)),
                    text_pair("original_checksum", checksum_value(&self.original_checksum)),
                    text_pair("newline_mode", CborValue::Text(self.newline_mode.clone())),
                    text_pair("line_count", u64_value(self.line_count)),
                ]),
            ),
            text_pair(
                "compression",
                CborValue::Map(vec![
                    text_pair("codec", CborValue::Text("zstd".to_owned())),
                    text_pair(
                        "zstd_level",
                        CborValue::Integer(i128::from(self.zstd_level)),
                    ),
                    text_pair("independent_frames", CborValue::Bool(true)),
                    text_pair("zstd_frame_checksum", CborValue::Bool(false)),
                    text_pair(
                        "dictionary_mode",
                        CborValue::Text(self.dictionary_mode.clone()),
                    ),
                ]),
            ),
            text_pair(
                "chunking",
                CborValue::Map(vec![
                    text_pair("target_chunk_size", u64_value(self.target_chunk_size)),
                    text_pair("max_chunk_size", u64_value(self.max_chunk_size)),
                    text_pair("boundary", CborValue::Text("line-preferred".to_owned())),
                    text_pair("utf8_boundary_required", CborValue::Bool(true)),
                ]),
            ),
            text_pair(
                "indexes",
                CborValue::Map(vec![
                    text_pair("chunk_table", CborValue::Bool(true)),
                    text_pair("sparse_line_index", CborValue::Bool(true)),
                    text_pair("dense_line_index", CborValue::Bool(self.dense_line_index)),
                    text_pair("document_index", CborValue::Bool(self.document_index)),
                    text_pair("token_index", CborValue::Bool(false)),
                    text_pair("ngram_index", CborValue::Bool(false)),
                    text_pair("vector_index", CborValue::Bool(false)),
                ]),
            ),
            text_pair(
                "integrity",
                CborValue::Map(vec![
                    text_pair(
                        "compressed_chunk_checksum",
                        CborValue::Text(CHECKSUM_ALGORITHM_BLAKE3.to_owned()),
                    ),
                    text_pair(
                        "uncompressed_chunk_checksum",
                        CborValue::Text(CHECKSUM_ALGORITHM_BLAKE3.to_owned()),
                    ),
                    text_pair(
                        "index_checksum",
                        CborValue::Text(CHECKSUM_ALGORITHM_BLAKE3.to_owned()),
                    ),
                ]),
            ),
            text_pair(
                "compatibility",
                CborValue::Map(vec![
                    text_pair("qzt_is_zst_stream", CborValue::Bool(false)),
                    text_pair("chunks_are_independent_zstd_frames", CborValue::Bool(true)),
                ]),
            ),
        ]))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let value = validate_deterministic(bytes)?;
        let map = as_map(&value, QztError::MetadataInvalid)?;
        reject_unknown_keys(
            map,
            &[
                "schema",
                "format",
                "format_version",
                "container_id",
                "identity",
                "source",
                "compression",
                "chunking",
                "indexes",
                "integrity",
                "compatibility",
            ],
            QztError::MetadataInvalid,
        )?;
        expect_text_field(map, "schema", SCHEMA_METADATA, QztError::MetadataInvalid)?;
        expect_text_field(map, "format", "qzt", QztError::MetadataInvalid)?;
        expect_version_field(map, QztError::MetadataInvalid)?;

        let identity = required_map(map, "identity", QztError::MetadataInvalid)?;
        reject_unknown_keys(
            identity,
            &["name", "profile", "created_by", "created_at_unix_ms"],
            QztError::MetadataInvalid,
        )?;

        let source = required_map(map, "source", QztError::MetadataInvalid)?;
        reject_unknown_keys(
            source,
            &[
                "media_type",
                "encoding",
                "original_size",
                "original_checksum",
                "newline_mode",
                "line_count",
            ],
            QztError::MetadataInvalid,
        )?;
        expect_text_field(source, "media_type", "text", QztError::MetadataInvalid)?;
        expect_text_field(source, "encoding", "utf-8", QztError::MetadataInvalid)?;

        let compression = required_map(map, "compression", QztError::MetadataInvalid)?;
        reject_unknown_keys(
            compression,
            &[
                "codec",
                "zstd_level",
                "independent_frames",
                "zstd_frame_checksum",
                "dictionary_mode",
            ],
            QztError::MetadataInvalid,
        )?;
        expect_text_field(compression, "codec", "zstd", QztError::MetadataInvalid)?;
        expect_bool_field(
            compression,
            "independent_frames",
            true,
            QztError::MetadataInvalid,
        )?;
        expect_bool_field(
            compression,
            "zstd_frame_checksum",
            false,
            QztError::MetadataInvalid,
        )?;

        let chunking = required_map(map, "chunking", QztError::MetadataInvalid)?;
        reject_unknown_keys(
            chunking,
            &[
                "target_chunk_size",
                "max_chunk_size",
                "boundary",
                "utf8_boundary_required",
            ],
            QztError::MetadataInvalid,
        )?;
        expect_bool_field(
            chunking,
            "utf8_boundary_required",
            true,
            QztError::MetadataInvalid,
        )?;
        expect_text_field(
            chunking,
            "boundary",
            "line-preferred",
            QztError::MetadataInvalid,
        )?;

        let indexes = required_map(map, "indexes", QztError::MetadataInvalid)?;
        reject_unknown_keys(
            indexes,
            &[
                "chunk_table",
                "sparse_line_index",
                "dense_line_index",
                "document_index",
                "token_index",
                "ngram_index",
                "vector_index",
            ],
            QztError::MetadataInvalid,
        )?;
        // Core containers always have a chunk_table and a sparse line index.
        // Extension indexes are carried by the sidecar, not the Core metadata,
        // so the Core metadata fields must remain false.
        expect_bool_field(indexes, "chunk_table", true, QztError::MetadataInvalid)?;
        expect_bool_field(
            indexes,
            "sparse_line_index",
            true,
            QztError::MetadataInvalid,
        )?;
        expect_bool_field(indexes, "token_index", false, QztError::MetadataInvalid)?;
        expect_bool_field(indexes, "ngram_index", false, QztError::MetadataInvalid)?;
        expect_bool_field(indexes, "vector_index", false, QztError::MetadataInvalid)?;

        let integrity = required_map(map, "integrity", QztError::MetadataInvalid)?;
        reject_unknown_keys(
            integrity,
            &[
                "compressed_chunk_checksum",
                "uncompressed_chunk_checksum",
                "index_checksum",
            ],
            QztError::MetadataInvalid,
        )?;
        // All checksum algorithms must be blake3 for v0.1.
        expect_text_field(
            integrity,
            "compressed_chunk_checksum",
            CHECKSUM_ALGORITHM_BLAKE3,
            QztError::MetadataInvalid,
        )?;
        expect_text_field(
            integrity,
            "uncompressed_chunk_checksum",
            CHECKSUM_ALGORITHM_BLAKE3,
            QztError::MetadataInvalid,
        )?;
        expect_text_field(
            integrity,
            "index_checksum",
            CHECKSUM_ALGORITHM_BLAKE3,
            QztError::MetadataInvalid,
        )?;

        let compatibility = required_map(map, "compatibility", QztError::MetadataInvalid)?;
        reject_unknown_keys(
            compatibility,
            &["qzt_is_zst_stream", "chunks_are_independent_zstd_frames"],
            QztError::MetadataInvalid,
        )?;
        expect_bool_field(
            compatibility,
            "qzt_is_zst_stream",
            false,
            QztError::MetadataInvalid,
        )?;
        expect_bool_field(
            compatibility,
            "chunks_are_independent_zstd_frames",
            true,
            QztError::MetadataInvalid,
        )?;

        let newline_mode = required_text(source, "newline_mode", QztError::MetadataInvalid)?;
        if !matches!(newline_mode.as_str(), "none" | "lf" | "crlf" | "mixed") {
            return Err(QztError::MetadataInvalid);
        }
        let dictionary_mode =
            required_text(compression, "dictionary_mode", QztError::MetadataInvalid)?;
        if !matches!(dictionary_mode.as_str(), "none" | "embedded") {
            return Err(QztError::MetadataInvalid);
        }
        let profile = required_text(identity, "profile", QztError::MetadataInvalid)?;
        if !matches!(
            profile.as_str(),
            "minimal" | "core" | "log" | "archive" | "memory"
        ) {
            return Err(QztError::MetadataInvalid);
        }
        let zstd_level = required_i32(compression, "zstd_level", QztError::MetadataInvalid)?;
        let target_chunk_size =
            required_u64(chunking, "target_chunk_size", QztError::MetadataInvalid)?;
        let max_chunk_size = required_u64(chunking, "max_chunk_size", QztError::MetadataInvalid)?;
        if target_chunk_size == 0 || max_chunk_size == 0 || target_chunk_size > max_chunk_size {
            return Err(QztError::MetadataInvalid);
        }

        Ok(Self {
            container_id: required_bstr16(map, "container_id", QztError::MetadataInvalid)?,
            original_size: required_u64(source, "original_size", QztError::MetadataInvalid)?,
            original_checksum: required_checksum(
                source,
                "original_checksum",
                QztError::MetadataInvalid,
            )?,
            newline_mode,
            line_count: required_u64(source, "line_count", QztError::MetadataInvalid)?,
            zstd_level,
            target_chunk_size,
            max_chunk_size,
            dictionary_mode,
            profile,
            dense_line_index: required_bool(
                indexes,
                "dense_line_index",
                QztError::MetadataInvalid,
            )?,
            document_index: required_bool(indexes, "document_index", QztError::MetadataInvalid)?,
        })
    }
}

/// Index Root block descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDescriptor {
    pub block_type: String,
    pub required: bool,
    pub offset: u64,
    pub size: u64,
    pub codec: String,
    pub checksum: Checksum,
    pub flags: u64,
}

impl BlockDescriptor {
    #[must_use]
    pub fn chunk_table(offset: u64, size: u64, checksum: Checksum) -> Self {
        Self {
            block_type: CHUNK_TABLE_TYPE.to_owned(),
            required: true,
            offset,
            size,
            codec: CHUNK_TABLE_CODEC.to_owned(),
            checksum,
            flags: 0,
        }
    }

    #[must_use]
    pub fn dictionary(offset: u64, size: u64, checksum: Checksum) -> Self {
        Self {
            block_type: DICTIONARY_TYPE.to_owned(),
            required: false,
            offset,
            size,
            codec: DICTIONARY_CODEC.to_owned(),
            checksum,
            flags: 0,
        }
    }

    #[must_use]
    pub fn dense_line_index(offset: u64, size: u64, checksum: Checksum) -> Self {
        Self {
            block_type: DENSE_LINE_INDEX_TYPE.to_owned(),
            required: false,
            offset,
            size,
            codec: DENSE_LINE_INDEX_CODEC.to_owned(),
            checksum,
            flags: 0,
        }
    }

    #[must_use]
    pub fn document_index(offset: u64, size: u64, checksum: Checksum) -> Self {
        Self {
            block_type: DOCUMENT_INDEX_TYPE.to_owned(),
            required: false,
            offset,
            size,
            codec: DOCUMENT_INDEX_CODEC.to_owned(),
            checksum,
            flags: 0,
        }
    }
}

/// Embedded dictionary block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryBlock {
    pub container_id: [u8; 16],
    pub dictionaries: Vec<DictionaryEntry>,
}

impl DictionaryBlock {
    pub fn encode(&self) -> Result<Vec<u8>> {
        encode_deterministic(&CborValue::Map(vec![
            text_pair("schema", CborValue::Text(SCHEMA_DICTIONARY.to_owned())),
            text_pair("format_version", version_value()),
            text_pair("container_id", CborValue::Bytes(self.container_id.to_vec())),
            text_pair(
                "dictionaries",
                CborValue::Array(
                    self.dictionaries
                        .iter()
                        .map(dictionary_entry_value)
                        .collect(),
                ),
            ),
        ]))
    }

    pub fn decode_with_limits(bytes: &[u8], max_dictionary_size: u64) -> Result<Self> {
        let value = validate_deterministic(bytes)?;
        let map = as_map(&value, QztError::ContainerCorrupt)?;
        reject_unknown_keys(
            map,
            &["schema", "format_version", "container_id", "dictionaries"],
            QztError::ContainerCorrupt,
        )?;
        expect_text_field(map, "schema", SCHEMA_DICTIONARY, QztError::ContainerCorrupt)?;
        expect_version_field(map, QztError::ContainerCorrupt)?;

        let dictionaries = required_array(map, "dictionaries", QztError::ContainerCorrupt)?;
        let mut seen = BTreeSet::new();
        let mut decoded = Vec::with_capacity(dictionaries.len());
        for dictionary in dictionaries {
            let entry = decode_dictionary_entry(dictionary, max_dictionary_size)?;
            if !seen.insert(entry.dictionary_id) {
                return Err(QztError::ContainerCorrupt);
            }
            decoded.push(entry);
        }

        Ok(Self {
            container_id: required_bstr16(map, "container_id", QztError::ContainerCorrupt)?,
            dictionaries: decoded,
        })
    }
}

/// One embedded zstd dictionary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryEntry {
    pub dictionary_id: u32,
    pub codec: String,
    pub bytes: Vec<u8>,
    pub checksum: Checksum,
}

/// Optional Document Index block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentIndex {
    pub container_id: [u8; 16],
    pub documents: Vec<DocumentEntry>,
}

impl DocumentIndex {
    pub fn encode(&self) -> Result<Vec<u8>> {
        encode_deterministic(&CborValue::Map(vec![
            text_pair("schema", CborValue::Text(SCHEMA_DOCUMENT_INDEX.to_owned())),
            text_pair("format_version", version_value()),
            text_pair("container_id", CborValue::Bytes(self.container_id.to_vec())),
            text_pair(
                "documents",
                CborValue::Array(self.documents.iter().map(document_entry_value).collect()),
            ),
        ]))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let value = validate_deterministic(bytes)?;
        let map = as_map(&value, QztError::ContainerCorrupt)?;
        reject_unknown_keys(
            map,
            &["schema", "format_version", "container_id", "documents"],
            QztError::ContainerCorrupt,
        )?;
        expect_text_field(
            map,
            "schema",
            SCHEMA_DOCUMENT_INDEX,
            QztError::ContainerCorrupt,
        )?;
        expect_version_field(map, QztError::ContainerCorrupt)?;

        let documents = required_array(map, "documents", QztError::ContainerCorrupt)?
            .iter()
            .map(decode_document_entry)
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            container_id: required_bstr16(map, "container_id", QztError::ContainerCorrupt)?,
            documents,
        })
    }
}

/// One document range over original bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentEntry {
    pub doc_id: String,
    pub doc_id_hash: [u8; 16],
    pub logical_offset: u64,
    pub byte_length: u64,
    pub first_line: u64,
    pub line_count: u64,
    pub chunk_start: u64,
    pub chunk_end: u64,
    pub checksum: Checksum,
}

impl DocumentEntry {
    /// Constructs a `DocumentEntry`, computing `doc_id_hash` automatically from `doc_id`.
    ///
    /// Callers do not need to depend on blake3 directly; the hash is always derived correctly
    /// from the provided id.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        doc_id: impl Into<String>,
        logical_offset: u64,
        byte_length: u64,
        first_line: u64,
        line_count: u64,
        chunk_start: u64,
        chunk_end: u64,
        checksum: Checksum,
    ) -> Self {
        let doc_id = doc_id.into();
        let mut doc_id_hash = [0_u8; 16];
        doc_id_hash.copy_from_slice(&blake3::hash(doc_id.as_bytes()).as_bytes()[..16]);
        Self {
            doc_id,
            doc_id_hash,
            logical_offset,
            byte_length,
            first_line,
            line_count,
            chunk_start,
            chunk_end,
            checksum,
        }
    }
}

/// Index Root logical model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexRoot {
    pub container_id: [u8; 16],
    pub blocks: Vec<BlockDescriptor>,
    pub original_size: u64,
    pub original_checksum: Checksum,
    pub chunk_count: u64,
    pub line_count: u64,
}

impl IndexRoot {
    pub fn encode(&self) -> Result<Vec<u8>> {
        encode_deterministic(&CborValue::Map(vec![
            text_pair("schema", CborValue::Text(SCHEMA_INDEX_ROOT.to_owned())),
            text_pair("format_version", version_value()),
            text_pair("container_id", CborValue::Bytes(self.container_id.to_vec())),
            text_pair(
                "blocks",
                CborValue::Array(self.blocks.iter().map(block_descriptor_value).collect()),
            ),
            text_pair(
                "content",
                CborValue::Map(vec![
                    text_pair("original_size", u64_value(self.original_size)),
                    text_pair("original_checksum", checksum_value(&self.original_checksum)),
                    text_pair("chunk_count", u64_value(self.chunk_count)),
                    text_pair("line_count", u64_value(self.line_count)),
                ]),
            ),
        ]))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let value = validate_deterministic(bytes)?;
        let map = as_map(&value, QztError::ContainerCorrupt)?;
        reject_unknown_keys(
            map,
            &[
                "schema",
                "format_version",
                "container_id",
                "blocks",
                "content",
            ],
            QztError::ContainerCorrupt,
        )?;
        expect_text_field(map, "schema", SCHEMA_INDEX_ROOT, QztError::ContainerCorrupt)?;
        expect_version_field(map, QztError::ContainerCorrupt)?;

        let blocks = required_array(map, "blocks", QztError::ContainerCorrupt)?
            .iter()
            .map(decode_block_descriptor)
            .collect::<Result<Vec<_>>>()?;

        let required_chunk_tables = blocks
            .iter()
            .filter(|block| block.required && block.block_type == CHUNK_TABLE_TYPE)
            .count();
        match required_chunk_tables {
            0 => return Err(QztError::MissingRequiredBlock),
            1 => {}
            _ => return Err(QztError::ContainerCorrupt),
        }

        let content = required_map(map, "content", QztError::ContainerCorrupt)?;
        reject_unknown_keys(
            content,
            &[
                "original_size",
                "original_checksum",
                "chunk_count",
                "line_count",
            ],
            QztError::ContainerCorrupt,
        )?;

        Ok(Self {
            container_id: required_bstr16(map, "container_id", QztError::ContainerCorrupt)?,
            blocks,
            original_size: required_u64(content, "original_size", QztError::ContainerCorrupt)?,
            original_checksum: required_checksum(
                content,
                "original_checksum",
                QztError::ContainerCorrupt,
            )?,
            chunk_count: required_u64(content, "chunk_count", QztError::ContainerCorrupt)?,
            line_count: required_u64(content, "line_count", QztError::ContainerCorrupt)?,
        })
    }

    pub fn chunk_table_block(&self) -> Result<&BlockDescriptor> {
        self.blocks
            .iter()
            .find(|block| block.required && block.block_type == CHUNK_TABLE_TYPE)
            .ok_or(QztError::MissingRequiredBlock)
    }
}

/// Ensures Metadata and Index Root describe the same source.
pub fn validate_source_consistency(metadata: &Metadata, index_root: &IndexRoot) -> Result<()> {
    if metadata.container_id != index_root.container_id {
        return Err(QztError::ContainerIdMismatch);
    }

    if metadata.original_size != index_root.original_size
        || metadata.original_checksum != index_root.original_checksum
        || metadata.line_count != index_root.line_count
    {
        return Err(QztError::MetadataInvalid);
    }

    Ok(())
}

fn decode_block_descriptor(value: &CborValue) -> Result<BlockDescriptor> {
    let map = as_map(value, QztError::ContainerCorrupt)?;
    reject_unknown_keys(
        map,
        &[
            "type", "required", "offset", "size", "codec", "checksum", "flags",
        ],
        QztError::ContainerCorrupt,
    )?;
    let block_type = required_text(map, "type", QztError::ContainerCorrupt)?;
    let required = required_bool(map, "required", QztError::ContainerCorrupt)?;
    let flags = required_u64(map, "flags", QztError::ContainerCorrupt)?;

    if flags != 0 {
        return Err(QztError::InvalidFlags);
    }

    // In v0.1 Core the only block that may carry required=true is chunk_table.
    // Every other block type — including extension types the reader knows by name
    // (token_index, ngram_index, etc.) but does not process — must not be marked
    // required, because the reader cannot satisfy the capability they imply.
    if required && block_type != CHUNK_TABLE_TYPE {
        return Err(QztError::UnknownRequiredBlock);
    }

    Ok(BlockDescriptor {
        block_type,
        required,
        offset: required_u64(map, "offset", QztError::ContainerCorrupt)?,
        size: required_u64(map, "size", QztError::ContainerCorrupt)?,
        codec: required_text(map, "codec", QztError::ContainerCorrupt)?,
        checksum: required_checksum(map, "checksum", QztError::ContainerCorrupt)?,
        flags,
    })
}

fn dictionary_entry_value(entry: &DictionaryEntry) -> CborValue {
    CborValue::Map(vec![
        text_pair(
            "dictionary_id",
            CborValue::Integer(i128::from(entry.dictionary_id)),
        ),
        text_pair("codec", CborValue::Text(entry.codec.clone())),
        text_pair("bytes", CborValue::Bytes(entry.bytes.clone())),
        text_pair("checksum", checksum_value(&entry.checksum)),
    ])
}

fn decode_dictionary_entry(value: &CborValue, max_dictionary_size: u64) -> Result<DictionaryEntry> {
    let map = as_map(value, QztError::ContainerCorrupt)?;
    reject_unknown_keys(
        map,
        &["dictionary_id", "codec", "bytes", "checksum"],
        QztError::ContainerCorrupt,
    )?;

    let dictionary_id = u32::try_from(required_u64(
        map,
        "dictionary_id",
        QztError::ContainerCorrupt,
    )?)
    .map_err(|_| QztError::ContainerCorrupt)?;
    if dictionary_id == 0 {
        return Err(QztError::ContainerCorrupt);
    }

    let codec = required_text(map, "codec", QztError::ContainerCorrupt)?;
    if codec != "zstd" {
        return Err(QztError::ContainerCorrupt);
    }

    let bytes = required_bytes(map, "bytes", QztError::ContainerCorrupt)?;
    if usize_to_u64(bytes.len())? > max_dictionary_size {
        return Err(QztError::ResourceLimitExceeded);
    }

    let checksum = required_checksum(map, "checksum", QztError::ContainerCorrupt)?;
    if Checksum::blake3(&bytes) != checksum {
        return Err(QztError::DictionaryChecksumMismatch);
    }

    Ok(DictionaryEntry {
        dictionary_id,
        codec,
        bytes,
        checksum,
    })
}

fn document_entry_value(entry: &DocumentEntry) -> CborValue {
    CborValue::Map(vec![
        text_pair("doc_id", CborValue::Text(entry.doc_id.clone())),
        text_pair("doc_id_hash", CborValue::Bytes(entry.doc_id_hash.to_vec())),
        text_pair("logical_offset", u64_value(entry.logical_offset)),
        text_pair("byte_length", u64_value(entry.byte_length)),
        text_pair("first_line", u64_value(entry.first_line)),
        text_pair("line_count", u64_value(entry.line_count)),
        text_pair("chunk_start", u64_value(entry.chunk_start)),
        text_pair("chunk_end", u64_value(entry.chunk_end)),
        text_pair("checksum", checksum_value(&entry.checksum)),
        text_pair("metadata", CborValue::Map(Vec::new())),
    ])
}

fn decode_document_entry(value: &CborValue) -> Result<DocumentEntry> {
    let map = as_map(value, QztError::ContainerCorrupt)?;
    reject_unknown_keys(
        map,
        &[
            "doc_id",
            "doc_id_hash",
            "logical_offset",
            "byte_length",
            "first_line",
            "line_count",
            "chunk_start",
            "chunk_end",
            "checksum",
            "metadata",
        ],
        QztError::ContainerCorrupt,
    )?;
    let _metadata = required_map(map, "metadata", QztError::ContainerCorrupt)?;

    Ok(DocumentEntry {
        doc_id: required_text(map, "doc_id", QztError::ContainerCorrupt)?,
        doc_id_hash: required_bstr16(map, "doc_id_hash", QztError::ContainerCorrupt)?,
        logical_offset: required_u64(map, "logical_offset", QztError::ContainerCorrupt)?,
        byte_length: required_u64(map, "byte_length", QztError::ContainerCorrupt)?,
        first_line: required_u64(map, "first_line", QztError::ContainerCorrupt)?,
        line_count: required_u64(map, "line_count", QztError::ContainerCorrupt)?,
        chunk_start: required_u64(map, "chunk_start", QztError::ContainerCorrupt)?,
        chunk_end: required_u64(map, "chunk_end", QztError::ContainerCorrupt)?,
        checksum: required_checksum(map, "checksum", QztError::ContainerCorrupt)?,
    })
}

fn block_descriptor_value(block: &BlockDescriptor) -> CborValue {
    CborValue::Map(vec![
        text_pair("type", CborValue::Text(block.block_type.clone())),
        text_pair("required", CborValue::Bool(block.required)),
        text_pair("offset", u64_value(block.offset)),
        text_pair("size", u64_value(block.size)),
        text_pair("codec", CborValue::Text(block.codec.clone())),
        text_pair("checksum", checksum_value(&block.checksum)),
        text_pair("flags", u64_value(block.flags)),
    ])
}

fn block_ref_value(block: &BlockRef) -> CborValue {
    CborValue::Map(vec![
        text_pair("offset", u64_value(block.offset)),
        text_pair("size", u64_value(block.size)),
        text_pair("checksum", checksum_value(&block.checksum)),
    ])
}

fn checksum_value(checksum: &Checksum) -> CborValue {
    CborValue::Map(vec![
        text_pair("algorithm", CborValue::Text(checksum.algorithm.clone())),
        text_pair("value", CborValue::Bytes(checksum.value.to_vec())),
    ])
}

fn version_value() -> CborValue {
    CborValue::Array(vec![CborValue::Integer(0), CborValue::Integer(1)])
}

fn text_pair(key: &str, value: CborValue) -> (CborValue, CborValue) {
    (CborValue::Text(key.to_owned()), value)
}

fn u64_value(value: u64) -> CborValue {
    CborValue::Integer(i128::from(value))
}

fn as_map(value: &CborValue, error: QztError) -> Result<&[(CborValue, CborValue)]> {
    match value {
        CborValue::Map(entries) => Ok(entries.as_slice()),
        _ => Err(error),
    }
}

fn field<'a>(
    map: &'a [(CborValue, CborValue)],
    key: &str,
    error: QztError,
) -> Result<&'a CborValue> {
    map.iter()
        .find_map(|(entry_key, value)| {
            (entry_key == &CborValue::Text(key.to_owned())).then_some(value)
        })
        .ok_or(error)
}

fn required_map<'a>(
    map: &'a [(CborValue, CborValue)],
    key: &str,
    error: QztError,
) -> Result<&'a [(CborValue, CborValue)]> {
    as_map(field(map, key, error)?, error)
}

fn required_array<'a>(
    map: &'a [(CborValue, CborValue)],
    key: &str,
    error: QztError,
) -> Result<&'a [CborValue]> {
    match field(map, key, error)? {
        CborValue::Array(values) => Ok(values),
        _ => Err(error),
    }
}

fn required_text(map: &[(CborValue, CborValue)], key: &str, error: QztError) -> Result<String> {
    match field(map, key, error)? {
        CborValue::Text(value) => Ok(value.clone()),
        _ => Err(error),
    }
}

fn required_bool(map: &[(CborValue, CborValue)], key: &str, error: QztError) -> Result<bool> {
    match field(map, key, error)? {
        CborValue::Bool(value) => Ok(*value),
        _ => Err(error),
    }
}

fn required_u64(map: &[(CborValue, CborValue)], key: &str, error: QztError) -> Result<u64> {
    match field(map, key, error)? {
        CborValue::Integer(value) => u64::try_from(*value).map_err(|_| error),
        _ => Err(error),
    }
}

fn required_i32(map: &[(CborValue, CborValue)], key: &str, error: QztError) -> Result<i32> {
    match field(map, key, error)? {
        CborValue::Integer(value) => i32::try_from(*value).map_err(|_| error),
        _ => Err(error),
    }
}

fn required_bstr16(map: &[(CborValue, CborValue)], key: &str, error: QztError) -> Result<[u8; 16]> {
    required_bstr::<16>(map, key, error)
}

fn required_bstr32(map: &[(CborValue, CborValue)], key: &str, error: QztError) -> Result<[u8; 32]> {
    required_bstr::<32>(map, key, error)
}

fn required_bytes(map: &[(CborValue, CborValue)], key: &str, error: QztError) -> Result<Vec<u8>> {
    match field(map, key, error)? {
        CborValue::Bytes(bytes) => Ok(bytes.clone()),
        _ => Err(error),
    }
}

fn required_bstr<const N: usize>(
    map: &[(CborValue, CborValue)],
    key: &str,
    error: QztError,
) -> Result<[u8; N]> {
    match field(map, key, error)? {
        CborValue::Bytes(bytes) => bytes.as_slice().try_into().map_err(|_| error),
        _ => Err(error),
    }
}

fn required_checksum(
    map: &[(CborValue, CborValue)],
    key: &str,
    error: QztError,
) -> Result<Checksum> {
    let checksum = required_map(map, key, error)?;
    reject_unknown_keys(checksum, &["algorithm", "value"], error)?;
    let algorithm = required_text(checksum, "algorithm", error)?;
    if algorithm != CHECKSUM_ALGORITHM_BLAKE3 {
        return Err(error);
    }

    Ok(Checksum::from_raw_bytes(required_bstr32(
        checksum, "value", error,
    )?))
}

fn optional_checksum(
    map: &[(CborValue, CborValue)],
    key: &str,
    error: QztError,
) -> Result<Option<Checksum>> {
    if !map
        .iter()
        .any(|(entry_key, _)| entry_key == &CborValue::Text(key.to_owned()))
    {
        return Ok(None);
    }

    required_checksum(map, key, error).map(Some)
}

fn required_block_ref(
    map: &[(CborValue, CborValue)],
    key: &str,
    error: QztError,
) -> Result<BlockRef> {
    let block = required_map(map, key, error)?;
    reject_unknown_keys(block, &["offset", "size", "checksum"], error)?;
    Ok(BlockRef {
        offset: required_u64(block, "offset", error)?,
        size: required_u64(block, "size", error)?,
        checksum: required_checksum(block, "checksum", error)?,
    })
}

fn reject_unknown_keys(
    map: &[(CborValue, CborValue)],
    allowed: &[&str],
    error: QztError,
) -> Result<()> {
    for (key, _) in map {
        let CborValue::Text(key) = key else {
            return Err(error);
        };

        if !allowed.contains(&key.as_str()) {
            return Err(error);
        }
    }
    Ok(())
}

fn expect_text_field(
    map: &[(CborValue, CborValue)],
    key: &str,
    expected: &str,
    error: QztError,
) -> Result<()> {
    if required_text(map, key, error)? != expected {
        return Err(error);
    }
    Ok(())
}

fn expect_bool_field(
    map: &[(CborValue, CborValue)],
    key: &str,
    expected: bool,
    error: QztError,
) -> Result<()> {
    if required_bool(map, key, error)? != expected {
        return Err(error);
    }
    Ok(())
}

fn expect_version_field(map: &[(CborValue, CborValue)], error: QztError) -> Result<()> {
    match field(map, "format_version", error)? {
        CborValue::Array(values)
            if values.as_slice() == [CborValue::Integer(0), CborValue::Integer(1)] =>
        {
            Ok(())
        }
        _ => Err(error),
    }
}
