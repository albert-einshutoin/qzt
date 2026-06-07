use crate::cbor::{encode_deterministic, validate_deterministic, CborValue};
use crate::error::{QztError, Result};
use std::collections::BTreeSet;

const SCHEMA_FOOTER: &str = "qzt.footer.v1";
const SCHEMA_METADATA: &str = "qzt.metadata.v1";
const SCHEMA_INDEX_ROOT: &str = "qzt.index-root.v1";
const SCHEMA_DICTIONARY: &str = "qzt.dictionary.v1";
const CHECKSUM_BLAKE3: &str = "blake3";
const CHUNK_TABLE_TYPE: &str = "chunk_table";
const CHUNK_TABLE_CODEC: &str = "qzt-ctbl-fixed-v1";
const DICTIONARY_TYPE: &str = "dictionary";
const DICTIONARY_CODEC: &str = "qzt-dict-cbor-v1";

/// BLAKE3 checksum value used by QZT Core structures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksum {
    pub algorithm: String,
    pub value: [u8; 32],
}

impl Checksum {
    #[must_use]
    pub fn blake3(bytes: &[u8]) -> Self {
        Self {
            algorithm: CHECKSUM_BLAKE3.to_owned(),
            value: *blake3::hash(bytes).as_bytes(),
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
    pub dictionary_mode: String,
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
        Self {
            container_id,
            original_size,
            original_checksum,
            newline_mode: newline_mode.to_owned(),
            line_count,
            dictionary_mode: dictionary_mode.to_owned(),
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
                    text_pair("profile", CborValue::Text("core".to_owned())),
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
                    text_pair("zstd_level", CborValue::Integer(0)),
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
                    text_pair("target_chunk_size", u64_value(4 * 1024 * 1024)),
                    text_pair("max_chunk_size", u64_value(16 * 1024 * 1024)),
                    text_pair("boundary", CborValue::Text("line-preferred".to_owned())),
                    text_pair("utf8_boundary_required", CborValue::Bool(true)),
                ]),
            ),
            text_pair(
                "indexes",
                CborValue::Map(vec![
                    text_pair("chunk_table", CborValue::Bool(true)),
                    text_pair("sparse_line_index", CborValue::Bool(true)),
                    text_pair("dense_line_index", CborValue::Bool(false)),
                    text_pair("document_index", CborValue::Bool(false)),
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
                        CborValue::Text(CHECKSUM_BLAKE3.to_owned()),
                    ),
                    text_pair(
                        "uncompressed_chunk_checksum",
                        CborValue::Text(CHECKSUM_BLAKE3.to_owned()),
                    ),
                    text_pair(
                        "index_checksum",
                        CborValue::Text(CHECKSUM_BLAKE3.to_owned()),
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
            dictionary_mode,
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

        if !blocks
            .iter()
            .any(|block| block.required && block.block_type == CHUNK_TABLE_TYPE)
        {
            return Err(QztError::MissingRequiredBlock);
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

    if required && !is_known_block_type(&block_type) {
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
    if u64::try_from(bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?
        > max_dictionary_size
    {
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

fn is_known_block_type(block_type: &str) -> bool {
    matches!(
        block_type,
        "metadata"
            | "chunk_table"
            | "dense_line_index"
            | "dictionary"
            | "document_index"
            | "token_index"
            | "ngram_index"
            | "optimizer_metadata"
            | "extension"
    )
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
    as_map(field(map, key, error.clone())?, error)
}

fn required_array<'a>(
    map: &'a [(CborValue, CborValue)],
    key: &str,
    error: QztError,
) -> Result<&'a [CborValue]> {
    match field(map, key, error.clone())? {
        CborValue::Array(values) => Ok(values),
        _ => Err(error),
    }
}

fn required_text(map: &[(CborValue, CborValue)], key: &str, error: QztError) -> Result<String> {
    match field(map, key, error.clone())? {
        CborValue::Text(value) => Ok(value.clone()),
        _ => Err(error),
    }
}

fn required_bool(map: &[(CborValue, CborValue)], key: &str, error: QztError) -> Result<bool> {
    match field(map, key, error.clone())? {
        CborValue::Bool(value) => Ok(*value),
        _ => Err(error),
    }
}

fn required_u64(map: &[(CborValue, CborValue)], key: &str, error: QztError) -> Result<u64> {
    match field(map, key, error.clone())? {
        CborValue::Integer(value) => u64::try_from(*value).map_err(|_| error),
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
    match field(map, key, error.clone())? {
        CborValue::Bytes(bytes) => Ok(bytes.clone()),
        _ => Err(error),
    }
}

fn required_bstr<const N: usize>(
    map: &[(CborValue, CborValue)],
    key: &str,
    error: QztError,
) -> Result<[u8; N]> {
    match field(map, key, error.clone())? {
        CborValue::Bytes(bytes) => bytes.as_slice().try_into().map_err(|_| error),
        _ => Err(error),
    }
}

fn required_checksum(
    map: &[(CborValue, CborValue)],
    key: &str,
    error: QztError,
) -> Result<Checksum> {
    let checksum = required_map(map, key, error.clone())?;
    reject_unknown_keys(checksum, &["algorithm", "value"], error.clone())?;
    let algorithm = required_text(checksum, "algorithm", error.clone())?;
    if algorithm != CHECKSUM_BLAKE3 {
        return Err(error);
    }

    Ok(Checksum {
        algorithm,
        value: required_bstr32(checksum, "value", error)?,
    })
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
    let block = required_map(map, key, error.clone())?;
    reject_unknown_keys(block, &["offset", "size", "checksum"], error.clone())?;
    Ok(BlockRef {
        offset: required_u64(block, "offset", error.clone())?,
        size: required_u64(block, "size", error.clone())?,
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
    if required_text(map, key, error.clone())? != expected {
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
    if required_bool(map, key, error.clone())? != expected {
        return Err(error);
    }
    Ok(())
}

fn expect_version_field(map: &[(CborValue, CborValue)], error: QztError) -> Result<()> {
    match field(map, "format_version", error.clone())? {
        CborValue::Array(values)
            if values.as_slice() == [CborValue::Integer(0), CborValue::Integer(1)] =>
        {
            Ok(())
        }
        _ => Err(error),
    }
}
