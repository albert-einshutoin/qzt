use crate::chunk_table::ChunkEntry;
use crate::chunker::{plan_chunks, ChunkerOptions, NewlineMode};
use crate::dense_line_index::DenseLineIndex;
use crate::error::{QztError, Result};
use crate::fixed::{FooterTrailer, Header};
use crate::format::{FOOTER_TRAILER_LEN, HEADER_LEN};
use crate::schema::{
    BlockDescriptor, BlockRef, Checksum, DocumentIndex, FooterPayload, IndexRoot, Metadata,
    MetadataOptions,
};

/// Placeholder writer entry point reserved for a future streaming API.
#[doc(hidden)]
pub struct QztWriter;

impl QztWriter {
    /// Creates a QZT writer.
    pub fn new() -> Result<Self> {
        Err(QztError::NotImplemented("QztWriter::new"))
    }
}

/// Writer options for the no-dictionary Phase5 writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriterOptions {
    pub chunker: ChunkerOptions,
    pub zstd_level: i32,
}

impl Default for WriterOptions {
    fn default() -> Self {
        Self {
            chunker: ChunkerOptions {
                target_chunk_size: 4 * 1024 * 1024,
                max_chunk_size: 16 * 1024 * 1024,
            },
            zstd_level: 0,
        }
    }
}

/// Packs UTF-8 input into a no-dictionary QZT container.
pub fn pack_bytes(input: &[u8], options: WriterOptions) -> Result<Vec<u8>> {
    let hash = blake3::hash(input);
    let mut container_id = [0_u8; 16];
    container_id.copy_from_slice(&hash.as_bytes()[..16]);
    pack_bytes_with_container_id(input, container_id, options)
}

/// Packs UTF-8 input with profile and optional Dense Line Index metadata.
pub fn pack_bytes_with_profile(
    input: &[u8],
    options: WriterOptions,
    profile: &str,
    dense_line_index: bool,
) -> Result<Vec<u8>> {
    let hash = blake3::hash(input);
    let mut container_id = [0_u8; 16];
    container_id.copy_from_slice(&hash.as_bytes()[..16]);
    pack_bytes_with_profile_and_container_id(
        input,
        container_id,
        options,
        profile,
        dense_line_index,
    )
}

/// Packs UTF-8 input with an explicit container id, profile, and optional Dense Line Index.
pub fn pack_bytes_with_profile_and_container_id(
    input: &[u8],
    container_id: [u8; 16],
    options: WriterOptions,
    profile: &str,
    dense_line_index: bool,
) -> Result<Vec<u8>> {
    validate_profile(profile)?;
    let dense_mode = if dense_line_index {
        DenseLineIndexMode::Generate
    } else {
        DenseLineIndexMode::Omit
    };
    pack_bytes_internal(input, container_id, options, dense_mode, None, profile)
}

/// Packs UTF-8 input with an explicit container id for deterministic tests.
pub fn pack_bytes_with_container_id(
    input: &[u8],
    container_id: [u8; 16],
    options: WriterOptions,
) -> Result<Vec<u8>> {
    pack_bytes_internal(
        input,
        container_id,
        options,
        DenseLineIndexMode::Omit,
        None,
        "core",
    )
}

/// Packs UTF-8 input with an optional Dense Line Index block.
pub fn pack_bytes_with_dense_line_index(
    input: &[u8],
    container_id: [u8; 16],
    options: WriterOptions,
) -> Result<Vec<u8>> {
    pack_bytes_internal(
        input,
        container_id,
        options,
        DenseLineIndexMode::Generate,
        None,
        "core",
    )
}

/// Packs UTF-8 input with a caller-provided Dense Line Index block.
///
/// This is primarily useful for conformance fixtures where the optional index
/// must be stale while the authoritative Chunk Table remains valid.
pub fn pack_bytes_with_dense_line_index_override(
    input: &[u8],
    container_id: [u8; 16],
    options: WriterOptions,
    dense_line_index: DenseLineIndex,
) -> Result<Vec<u8>> {
    pack_bytes_internal(
        input,
        container_id,
        options,
        DenseLineIndexMode::Override(dense_line_index),
        None,
        "core",
    )
}

/// Packs UTF-8 input with an optional Document Index block.
pub fn pack_bytes_with_document_index(
    input: &[u8],
    container_id: [u8; 16],
    options: WriterOptions,
    document_index: DocumentIndex,
) -> Result<Vec<u8>> {
    pack_bytes_internal(
        input,
        container_id,
        options,
        DenseLineIndexMode::Omit,
        Some(document_index),
        "core",
    )
}

/// Packs UTF-8 input using the memory profile defaults implemented in Phase10.
pub fn pack_bytes_with_memory_profile(
    input: &[u8],
    container_id: [u8; 16],
    options: WriterOptions,
    document_index: DocumentIndex,
) -> Result<Vec<u8>> {
    pack_bytes_internal(
        input,
        container_id,
        options,
        DenseLineIndexMode::Generate,
        Some(document_index),
        "memory",
    )
}

enum DenseLineIndexMode {
    Omit,
    Generate,
    Override(DenseLineIndex),
}

struct OptionalBlocks<'a> {
    dense_line_index: Option<&'a DenseLineIndex>,
    document_index: Option<&'a DocumentIndex>,
    profile: &'a str,
    writer_options: WriterOptions,
}

fn pack_bytes_internal(
    input: &[u8],
    container_id: [u8; 16],
    options: WriterOptions,
    dense_mode: DenseLineIndexMode,
    document_index: Option<DocumentIndex>,
    profile: &str,
) -> Result<Vec<u8>> {
    let plan = plan_chunks(input, options.chunker)?;
    let mut compressed_chunks = Vec::with_capacity(plan.chunks.len());
    let mut entries = Vec::with_capacity(plan.chunks.len());
    let mut physical_offset = HEADER_LEN as u64;

    for chunk in &plan.chunks {
        let start =
            usize::try_from(chunk.logical_offset).map_err(|_| QztError::ResourceLimitExceeded)?;
        let end = start
            .checked_add(
                usize::try_from(chunk.uncompressed_size)
                    .map_err(|_| QztError::ResourceLimitExceeded)?,
            )
            .ok_or(QztError::ResourceLimitExceeded)?;
        let uncompressed = input.get(start..end).ok_or(QztError::ContainerCorrupt)?;
        let compressed = zstd::stream::encode_all(uncompressed, options.zstd_level)
            .map_err(|_| QztError::ZstdEncodeError)?;

        if compressed.is_empty() {
            return Err(QztError::ChunkSizeMismatch);
        }

        let compressed_size =
            u64::try_from(compressed.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
        let entry = ChunkEntry {
            chunk_id: chunk.chunk_id,
            physical_offset,
            compressed_size,
            logical_offset: chunk.logical_offset,
            uncompressed_size: chunk.uncompressed_size,
            first_line: chunk.first_line,
            line_count: chunk.line_count,
            dictionary_id: 0,
            flags: chunk.flags,
            compressed_checksum_blake3: Checksum::blake3(&compressed).value,
            uncompressed_checksum_blake3: Checksum::blake3(uncompressed).value,
        };

        physical_offset = physical_offset
            .checked_add(compressed_size)
            .ok_or(QztError::PhysicalRangeOutOfBounds)?;
        entries.push(entry);
        compressed_chunks.push(compressed);
    }

    let dense_line_index = match dense_mode {
        DenseLineIndexMode::Omit => None,
        DenseLineIndexMode::Generate => Some(DenseLineIndex::from_original_bytes(input, &entries)?),
        DenseLineIndexMode::Override(dense) => Some(dense),
    };

    assemble_container(
        input,
        container_id,
        &plan,
        &compressed_chunks,
        &entries,
        OptionalBlocks {
            dense_line_index: dense_line_index.as_ref(),
            document_index: document_index.as_ref(),
            profile,
            writer_options: options,
        },
    )
}

/// Exports all original bytes from a no-dictionary QZT container.
pub fn export_all(container: &[u8]) -> Result<Vec<u8>> {
    crate::reader::QztReader::open(container)?.export_all()
}

fn assemble_container(
    input: &[u8],
    container_id: [u8; 16],
    plan: &crate::chunker::ChunkPlan,
    compressed_chunks: &[Vec<u8>],
    entries: &[ChunkEntry],
    optional: OptionalBlocks<'_>,
) -> Result<Vec<u8>> {
    if compressed_chunks.len() != entries.len() {
        return Err(QztError::ContainerCorrupt);
    }

    let metadata_offset = entries
        .last()
        .map(|entry| {
            entry
                .physical_offset
                .checked_add(entry.compressed_size)
                .ok_or(QztError::PhysicalRangeOutOfBounds)
        })
        .transpose()?
        .unwrap_or(HEADER_LEN as u64);
    let metadata = Metadata::for_source_with_options(
        container_id,
        u64::try_from(input.len()).map_err(|_| QztError::ResourceLimitExceeded)?,
        Checksum::blake3(input),
        newline_mode_as_str(plan.newline_mode),
        plan.line_count,
        MetadataOptions {
            zstd_level: optional.writer_options.zstd_level,
            target_chunk_size: u64::try_from(optional.writer_options.chunker.target_chunk_size)
                .map_err(|_| QztError::ResourceLimitExceeded)?,
            max_chunk_size: u64::try_from(optional.writer_options.chunker.max_chunk_size)
                .map_err(|_| QztError::ResourceLimitExceeded)?,
            dictionary_mode: "none",
            profile: optional.profile,
            dense_line_index: optional.dense_line_index.is_some(),
            document_index: optional.document_index.is_some(),
        },
    );
    let metadata_bytes = metadata.encode()?;
    let metadata_size =
        u64::try_from(metadata_bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?;

    let dense_line_index_bytes = optional
        .dense_line_index
        .map(DenseLineIndex::encode)
        .transpose()?;
    let dense_line_index_offset = metadata_offset
        .checked_add(metadata_size)
        .ok_or(QztError::PhysicalRangeOutOfBounds)?;
    let dense_line_index_size = dense_line_index_bytes
        .as_ref()
        .map(|bytes| u64::try_from(bytes.len()).map_err(|_| QztError::ResourceLimitExceeded))
        .transpose()?
        .unwrap_or(0);

    let document_index_bytes = optional
        .document_index
        .map(DocumentIndex::encode)
        .transpose()?;
    let document_index_offset = dense_line_index_offset
        .checked_add(dense_line_index_size)
        .ok_or(QztError::PhysicalRangeOutOfBounds)?;
    let document_index_size = document_index_bytes
        .as_ref()
        .map(|bytes| u64::try_from(bytes.len()).map_err(|_| QztError::ResourceLimitExceeded))
        .transpose()?
        .unwrap_or(0);

    let mut chunk_table_bytes =
        Vec::with_capacity(entries.len() * crate::chunk_table::CHUNK_ENTRY_LEN);
    for entry in entries {
        chunk_table_bytes.extend_from_slice(&entry.encode());
    }
    let chunk_table_offset = document_index_offset
        .checked_add(document_index_size)
        .ok_or(QztError::PhysicalRangeOutOfBounds)?;
    let chunk_table_size =
        u64::try_from(chunk_table_bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?;

    let mut blocks = vec![BlockDescriptor::chunk_table(
        chunk_table_offset,
        chunk_table_size,
        Checksum::blake3(&chunk_table_bytes),
    )];
    if let Some(bytes) = &dense_line_index_bytes {
        blocks.push(BlockDescriptor::dense_line_index(
            dense_line_index_offset,
            u64::try_from(bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?,
            Checksum::blake3(bytes),
        ));
    }
    if let Some(bytes) = &document_index_bytes {
        blocks.push(BlockDescriptor::document_index(
            document_index_offset,
            u64::try_from(bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?,
            Checksum::blake3(bytes),
        ));
    }

    let index_root = IndexRoot {
        container_id,
        blocks,
        original_size: metadata.original_size,
        original_checksum: metadata.original_checksum.clone(),
        chunk_count: u64::try_from(entries.len()).map_err(|_| QztError::ResourceLimitExceeded)?,
        line_count: metadata.line_count,
    };
    let index_root_bytes = index_root.encode()?;
    let index_root_offset = chunk_table_offset
        .checked_add(chunk_table_size)
        .ok_or(QztError::PhysicalRangeOutOfBounds)?;
    let index_root_size =
        u64::try_from(index_root_bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
    let footer_payload_offset = index_root_offset
        .checked_add(index_root_size)
        .ok_or(QztError::PhysicalRangeOutOfBounds)?;

    let header = Header {
        metadata_offset,
        metadata_size,
        index_hint_offset: index_root_offset,
        container_id,
    };

    let mut prefix = Vec::new();
    prefix.extend_from_slice(&header.encode());
    for chunk in compressed_chunks {
        prefix.extend_from_slice(chunk);
    }
    prefix.extend_from_slice(&metadata_bytes);
    if let Some(bytes) = &dense_line_index_bytes {
        prefix.extend_from_slice(bytes);
    }
    if let Some(bytes) = &document_index_bytes {
        prefix.extend_from_slice(bytes);
    }
    prefix.extend_from_slice(&chunk_table_bytes);
    prefix.extend_from_slice(&index_root_bytes);

    let footer_payload = fixed_point_footer_payload(
        container_id,
        BlockRef {
            offset: index_root_offset,
            size: index_root_size,
            checksum: Checksum::blake3(&index_root_bytes),
        },
        BlockRef {
            offset: metadata_offset,
            size: metadata_size,
            checksum: Checksum::blake3(&metadata_bytes),
        },
        footer_payload_offset,
        Some(Checksum::blake3(&prefix)),
    )?;
    let footer_payload_bytes = footer_payload.encode()?;
    let footer_trailer = FooterTrailer {
        footer_payload_offset,
        footer_payload_size: u64::try_from(footer_payload_bytes.len())
            .map_err(|_| QztError::ResourceLimitExceeded)?,
        footer_payload_checksum_blake3: Checksum::blake3(&footer_payload_bytes).value,
    };

    let mut bytes = prefix;
    bytes.extend_from_slice(&footer_payload_bytes);
    bytes.extend_from_slice(&footer_trailer.encode());
    Ok(bytes)
}

fn fixed_point_footer_payload(
    container_id: [u8; 16],
    index_root: BlockRef,
    metadata: BlockRef,
    footer_payload_offset: u64,
    container_checksum: Option<Checksum>,
) -> Result<FooterPayload> {
    let mut final_file_size = 0_u64;

    // The footer includes `final_file_size`, so its encoded CBOR size may grow
    // when the file size crosses an integer-width boundary. Re-encoding to a
    // fixed point keeps the trailer offsets deterministic without reserving
    // padding in the format.
    for _ in 0..8 {
        let candidate = FooterPayload {
            container_id,
            index_root: index_root.clone(),
            metadata: metadata.clone(),
            final_file_size,
            footer_flags: 0,
            container_checksum: container_checksum.clone(),
        };
        let size = u64::try_from(candidate.encode()?.len())
            .map_err(|_| QztError::ResourceLimitExceeded)?;
        let next = footer_payload_offset
            .checked_add(size)
            .and_then(|value| value.checked_add(FOOTER_TRAILER_LEN as u64))
            .ok_or(QztError::PhysicalRangeOutOfBounds)?;

        if next == final_file_size {
            return Ok(candidate);
        }

        final_file_size = next;
    }

    Err(QztError::ContainerCorrupt)
}

fn validate_profile(profile: &str) -> Result<()> {
    if matches!(profile, "minimal" | "core" | "log" | "archive" | "memory") {
        Ok(())
    } else {
        Err(QztError::MetadataInvalid)
    }
}

fn newline_mode_as_str(mode: NewlineMode) -> &'static str {
    match mode {
        NewlineMode::None => "none",
        NewlineMode::Lf => "lf",
        NewlineMode::Crlf => "crlf",
        NewlineMode::Mixed => "mixed",
    }
}
