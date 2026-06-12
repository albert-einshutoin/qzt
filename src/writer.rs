use std::io::{Read, Seek, SeekFrom, Write};

use crate::chunk_table::ChunkEntry;
use crate::chunker::{plan_chunks, ChunkerOptions, NewlineMode};
use crate::dense_line_index::line_start_offsets;
use crate::dense_line_index::DenseLineIndex;
use crate::error::{QztError, Result};
use crate::fixed::{FooterTrailer, Header};
use crate::format::{FOOTER_TRAILER_LEN, HEADER_LEN};
use crate::primitives::{u64_to_usize, usize_to_u64};
use crate::schema::{
    BlockDescriptor, BlockRef, Checksum, DocumentIndex, FooterPayload, IndexRoot, Metadata,
    MetadataOptions,
};

/// Streaming QZT writer over a readable, writable, seekable output.
pub struct QztFileWriter<W: Read + Write + Seek> {
    writer: W,
    options: WriterOptions,
    pending: Vec<u8>,
    entries: Vec<ChunkEntry>,
    input_hasher: blake3::Hasher,
    physical_offset: u64,
    logical_offset: u64,
    line_starts_seen: u64,
    lf_count: u64,
    crlf_count: u64,
    previous_byte: Option<u8>,
    finished: bool,
    poisoned: bool,
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

/// Builder for QZT container bytes.
#[derive(Debug, Clone)]
pub struct WriterBuilder {
    options: WriterOptions,
    profile: String,
    dense_line_index: bool,
    container_id: Option<[u8; 16]>,
    document_index: Option<DocumentIndex>,
}

impl Default for WriterBuilder {
    fn default() -> Self {
        Self {
            options: WriterOptions::default(),
            profile: "core".to_owned(),
            dense_line_index: false,
            container_id: None,
            document_index: None,
        }
    }
}

impl WriterBuilder {
    /// Creates a builder with default writer options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets writer options.
    #[must_use]
    pub fn options(mut self, options: WriterOptions) -> Self {
        self.options = options;
        self
    }

    /// Sets a deterministic container id.
    #[must_use]
    pub fn container_id(mut self, container_id: [u8; 16]) -> Self {
        self.container_id = Some(container_id);
        self
    }

    /// Sets the metadata profile.
    #[must_use]
    pub fn profile(mut self, profile: impl Into<String>) -> Self {
        self.profile = profile.into();
        self
    }

    /// Enables or disables the optional Dense Line Index.
    #[must_use]
    pub fn dense_line_index(mut self, enabled: bool) -> Self {
        self.dense_line_index = enabled;
        self
    }

    /// Adds an optional Document Index block.
    #[must_use]
    pub fn document_index(mut self, document_index: DocumentIndex) -> Self {
        self.document_index = Some(document_index);
        self
    }

    /// Packs input bytes into a QZT container.
    pub fn pack(self, input: &[u8]) -> Result<Vec<u8>> {
        let document_index = self.document_index;
        let container_id = self.container_id.unwrap_or_else(|| {
            let hash = blake3::hash(input);
            let mut container_id = [0_u8; 16];
            container_id.copy_from_slice(&hash.as_bytes()[..16]);
            container_id
        });

        let document_index = match (self.profile.as_str(), document_index) {
            ("memory", Some(document_index)) => {
                return pack_bytes_with_memory_profile(
                    input,
                    container_id,
                    self.options,
                    &document_index,
                );
            }
            (_, document_index) => document_index,
        };

        let dense_mode = if self.dense_line_index {
            DenseLineIndexMode::Generate
        } else {
            DenseLineIndexMode::Omit
        };
        pack_bytes_internal(
            input,
            container_id,
            self.options,
            dense_mode,
            document_index.as_ref(),
            &self.profile,
        )
    }
}

impl<W: Read + Write + Seek> QztFileWriter<W> {
    /// Creates a streaming writer and reserves the fixed header.
    pub fn new(mut writer: W, options: WriterOptions) -> Result<Self> {
        options.chunker.validate()?;
        writer
            .seek(SeekFrom::Start(0))
            .map_err(|_| QztError::ContainerCorrupt)?;
        writer
            .write_all(&[0_u8; HEADER_LEN])
            .map_err(|_| QztError::ContainerCorrupt)?;
        Ok(Self {
            writer,
            options,
            pending: Vec::new(),
            entries: Vec::new(),
            input_hasher: blake3::Hasher::new(),
            physical_offset: HEADER_LEN as u64,
            logical_offset: 0,
            line_starts_seen: 0,
            lf_count: 0,
            crlf_count: 0,
            previous_byte: None,
            finished: false,
            poisoned: false,
        })
    }

    /// Pushes original UTF-8 bytes into the container stream.
    pub fn push(&mut self, bytes: &[u8]) -> Result<()> {
        if self.finished || self.poisoned {
            return Err(QztError::WriterAlreadyFinished);
        }
        let result = (|| {
            self.input_hasher.update(bytes);
            self.pending.extend_from_slice(bytes);
            while self.pending.len() > self.options.chunker.max_chunk_size {
                let end = choose_stream_chunk_end(&self.pending, self.options.chunker)?;
                self.emit_pending_chunk(end)?;
            }
            Ok(())
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    /// Finishes the immutable container and patches the header.
    pub fn finish(&mut self) -> Result<()> {
        if self.finished || self.poisoned {
            return Err(QztError::WriterAlreadyFinished);
        }
        let result = self.finish_inner();
        if result.is_err() {
            self.poisoned = true;
        } else {
            self.finished = true;
        }
        result
    }

    /// Returns the wrapped writer.
    pub fn into_inner(self) -> W {
        self.writer
    }

    fn finish_inner(&mut self) -> Result<()> {
        while !self.pending.is_empty() {
            let end = choose_known_chunk_end(&self.pending, self.options.chunker)?;
            self.emit_pending_chunk(end)?;
        }

        let input_hash = self.input_hasher.finalize();
        let mut container_id = [0_u8; 16];
        container_id.copy_from_slice(&input_hash.as_bytes()[..16]);
        let original_checksum = Checksum::from_raw_bytes(*input_hash.as_bytes());

        let metadata_offset = self.physical_offset;
        let metadata = Metadata::for_source_with_options(
            container_id,
            self.logical_offset,
            original_checksum,
            streaming_newline_mode_as_str(self.lf_count, self.crlf_count),
            self.line_starts_seen,
            MetadataOptions {
                zstd_level: self.options.zstd_level,
                target_chunk_size: usize_to_u64(self.options.chunker.target_chunk_size)?,
                max_chunk_size: usize_to_u64(self.options.chunker.max_chunk_size)?,
                dictionary_mode: "none",
                // The streaming writer is always "core"; profile is intentionally
                // non-configurable here. Profile validation lives in pack_bytes_internal.
                profile: "core",
                dense_line_index: false,
                document_index: false,
            },
        );
        let metadata_bytes = metadata.encode()?;
        let metadata_size = usize_to_u64(metadata_bytes.len())?;

        let chunk_table_offset = metadata_offset
            .checked_add(metadata_size)
            .ok_or(QztError::PhysicalRangeOutOfBounds)?;
        let mut chunk_table_bytes =
            Vec::with_capacity(self.entries.len() * crate::chunk_table::CHUNK_ENTRY_LEN);
        for entry in &self.entries {
            chunk_table_bytes.extend_from_slice(&entry.encode());
        }
        let chunk_table_size = usize_to_u64(chunk_table_bytes.len())?;

        let index_root = IndexRoot {
            container_id,
            blocks: vec![BlockDescriptor::chunk_table(
                chunk_table_offset,
                chunk_table_size,
                Checksum::blake3(&chunk_table_bytes),
            )],
            original_size: metadata.original_size,
            original_checksum: metadata.original_checksum.clone(),
            chunk_count: usize_to_u64(self.entries.len())?,
            line_count: metadata.line_count,
        };
        let index_root_bytes = index_root.encode()?;
        let index_root_offset = chunk_table_offset
            .checked_add(chunk_table_size)
            .ok_or(QztError::PhysicalRangeOutOfBounds)?;
        let index_root_size = usize_to_u64(index_root_bytes.len())?;
        let footer_payload_offset = index_root_offset
            .checked_add(index_root_size)
            .ok_or(QztError::PhysicalRangeOutOfBounds)?;
        let header = Header {
            metadata_offset,
            metadata_size,
            index_hint_offset: index_root_offset,
            container_id,
        };

        self.writer
            .seek(SeekFrom::Start(metadata_offset))
            .map_err(|_| QztError::ContainerCorrupt)?;
        self.writer
            .write_all(&metadata_bytes)
            .map_err(|_| QztError::ContainerCorrupt)?;
        self.writer
            .write_all(&chunk_table_bytes)
            .map_err(|_| QztError::ContainerCorrupt)?;
        self.writer
            .write_all(&index_root_bytes)
            .map_err(|_| QztError::ContainerCorrupt)?;
        self.writer
            .seek(SeekFrom::Start(0))
            .map_err(|_| QztError::ContainerCorrupt)?;
        self.writer
            .write_all(&header.encode())
            .map_err(|_| QztError::ContainerCorrupt)?;

        let container_checksum = self.hash_prefix(footer_payload_offset)?;

        let index_root_ref = BlockRef {
            offset: index_root_offset,
            size: index_root_size,
            checksum: Checksum::blake3(&index_root_bytes),
        };
        let metadata_ref = BlockRef {
            offset: metadata_offset,
            size: metadata_size,
            checksum: Checksum::blake3(&metadata_bytes),
        };
        let footer_payload = fixed_point_footer_payload(
            container_id,
            &index_root_ref,
            &metadata_ref,
            footer_payload_offset,
            Some(&container_checksum),
        )?;
        let footer_payload_bytes = footer_payload.encode()?;
        let footer_trailer = FooterTrailer {
            footer_payload_offset,
            footer_payload_size: usize_to_u64(footer_payload_bytes.len())?,
            footer_payload_checksum_blake3: Checksum::blake3(&footer_payload_bytes).value,
        };

        self.writer
            .seek(SeekFrom::Start(footer_payload_offset))
            .map_err(|_| QztError::ContainerCorrupt)?;
        self.writer
            .write_all(&footer_payload_bytes)
            .map_err(|_| QztError::ContainerCorrupt)?;
        self.writer
            .write_all(&footer_trailer.encode())
            .map_err(|_| QztError::ContainerCorrupt)?;
        self.writer
            .seek(SeekFrom::End(0))
            .map_err(|_| QztError::ContainerCorrupt)?;
        Ok(())
    }

    fn hash_prefix(&mut self, prefix_len: u64) -> Result<Checksum> {
        let mut hasher = blake3::Hasher::new();
        let mut remaining = prefix_len;
        let mut buffer = vec![0_u8; 64 * 1024];
        self.writer
            .seek(SeekFrom::Start(0))
            .map_err(|_| QztError::ContainerCorrupt)?;
        while remaining > 0 {
            let chunk_len = u64_to_usize(remaining.min(buffer.len() as u64))?;
            self.writer
                .read_exact(&mut buffer[..chunk_len])
                .map_err(|_| QztError::ContainerCorrupt)?;
            hasher.update(&buffer[..chunk_len]);
            remaining -= chunk_len as u64;
        }
        Ok(Checksum::from_hasher(&hasher))
    }

    fn emit_pending_chunk(&mut self, end: usize) -> Result<()> {
        let chunk = self
            .pending
            .get(..end)
            .ok_or(QztError::ResourceLimitExceeded)?
            .to_vec();
        self.emit_chunk(&chunk)?;
        self.pending.drain(..end);
        Ok(())
    }

    fn emit_chunk(&mut self, uncompressed: &[u8]) -> Result<()> {
        std::str::from_utf8(uncompressed).map_err(|_| QztError::InvalidUtf8)?;
        let compressed = zstd::stream::encode_all(uncompressed, self.options.zstd_level)
            .map_err(|_| QztError::ZstdEncodeError)?;
        if compressed.is_empty() {
            return Err(QztError::ChunkSizeMismatch);
        }

        let compressed_size = usize_to_u64(compressed.len())?;
        let flags = if self.logical_offset > 0 && self.previous_byte != Some(b'\n') {
            crate::chunk_table::STARTS_WITH_LINE_CONTINUATION
        } else {
            0
        };
        let line_count = usize_to_u64(line_start_offsets(uncompressed, flags)?.len())?;
        let entry = ChunkEntry {
            chunk_id: usize_to_u64(self.entries.len())?,
            physical_offset: self.physical_offset,
            compressed_size,
            logical_offset: self.logical_offset,
            uncompressed_size: usize_to_u64(uncompressed.len())?,
            first_line: self.line_starts_seen,
            line_count,
            dictionary_id: 0,
            flags,
            compressed_checksum_blake3: Checksum::blake3(&compressed).value,
            uncompressed_checksum_blake3: Checksum::blake3(uncompressed).value,
        };

        self.writer
            .seek(SeekFrom::Start(self.physical_offset))
            .map_err(|_| QztError::ContainerCorrupt)?;
        self.writer
            .write_all(&compressed)
            .map_err(|_| QztError::ContainerCorrupt)?;

        self.update_newline_state(uncompressed)?;
        self.line_starts_seen = self
            .line_starts_seen
            .checked_add(line_count)
            .ok_or(QztError::ResourceLimitExceeded)?;
        self.logical_offset = self
            .logical_offset
            .checked_add(entry.uncompressed_size)
            .ok_or(QztError::ResourceLimitExceeded)?;
        self.physical_offset = self
            .physical_offset
            .checked_add(compressed_size)
            .ok_or(QztError::PhysicalRangeOutOfBounds)?;
        self.entries.push(entry);
        Ok(())
    }

    fn update_newline_state(&mut self, bytes: &[u8]) -> Result<()> {
        for byte in bytes {
            if *byte == b'\n' {
                if self.previous_byte == Some(b'\r') {
                    self.crlf_count = self
                        .crlf_count
                        .checked_add(1)
                        .ok_or(QztError::ResourceLimitExceeded)?;
                } else {
                    self.lf_count = self
                        .lf_count
                        .checked_add(1)
                        .ok_or(QztError::ResourceLimitExceeded)?;
                }
            }
            self.previous_byte = Some(*byte);
        }
        Ok(())
    }
}

fn choose_stream_chunk_end(input: &[u8], options: ChunkerOptions) -> Result<usize> {
    let max_end = options.max_chunk_size;
    if input.len() <= max_end {
        return Err(QztError::ResourceLimitExceeded);
    }
    choose_non_final_chunk_end(input, options.target_chunk_size, max_end)
}

fn choose_known_chunk_end(input: &[u8], options: ChunkerOptions) -> Result<usize> {
    if input.len() <= options.target_chunk_size {
        return Ok(input.len());
    }
    let max_end = options.max_chunk_size.min(input.len());
    choose_non_final_chunk_end(input, options.target_chunk_size, max_end)
}

fn choose_non_final_chunk_end(input: &[u8], target_end: usize, max_end: usize) -> Result<usize> {
    if let Some(line_end) = last_line_boundary(input, target_end) {
        return Ok(line_end);
    }
    if let Some(line_end) = last_line_boundary(input, max_end) {
        return Ok(line_end);
    }
    previous_valid_split(input, max_end).ok_or(QztError::ResourceLimitExceeded)
}

fn last_line_boundary(input: &[u8], end: usize) -> Option<usize> {
    let mut cursor = 0_usize;
    let mut boundary = None;
    while cursor < end {
        if input[cursor] == b'\n' {
            boundary = Some(cursor + 1);
        }
        cursor += 1;
    }
    boundary.filter(|candidate| *candidate > 0 && !splits_crlf(input, *candidate))
}

fn previous_valid_split(input: &[u8], max_end: usize) -> Option<usize> {
    (1..=max_end)
        .rev()
        .find(|candidate| is_utf8_boundary(input, *candidate) && !splits_crlf(input, *candidate))
}

fn is_utf8_boundary(input: &[u8], index: usize) -> bool {
    index == 0
        || index == input.len()
        || input
            .get(index)
            .is_some_and(|byte| byte & 0b1100_0000 != 0b1000_0000)
}

fn splits_crlf(input: &[u8], end: usize) -> bool {
    end > 0 && end < input.len() && input[end - 1] == b'\r' && input[end] == b'\n'
}

fn streaming_newline_mode_as_str(lf_count: u64, crlf_count: u64) -> &'static str {
    match (lf_count > 0, crlf_count > 0) {
        (false, false) => "none",
        (true, false) => "lf",
        (false, true) => "crlf",
        (true, true) => "mixed",
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
    document_index: &DocumentIndex,
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
    document_index: &DocumentIndex,
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
    document_index: Option<&DocumentIndex>,
    profile: &str,
) -> Result<Vec<u8>> {
    validate_profile(profile)?;
    if profile == "memory" && document_index.is_none() {
        return Err(QztError::MetadataInvalid);
    }
    let plan = plan_chunks(input, options.chunker)?;
    let mut compressed_chunks = Vec::with_capacity(plan.chunks.len());
    let mut entries = Vec::with_capacity(plan.chunks.len());
    let mut physical_offset = HEADER_LEN as u64;

    for chunk in &plan.chunks {
        let start = u64_to_usize(chunk.logical_offset)?;
        let end = start
            .checked_add(u64_to_usize(chunk.uncompressed_size)?)
            .ok_or(QztError::ResourceLimitExceeded)?;
        let uncompressed = input.get(start..end).ok_or(QztError::ContainerCorrupt)?;
        let compressed = zstd::stream::encode_all(uncompressed, options.zstd_level)
            .map_err(|_| QztError::ZstdEncodeError)?;

        if compressed.is_empty() {
            return Err(QztError::ChunkSizeMismatch);
        }

        let compressed_size = usize_to_u64(compressed.len())?;
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
        &OptionalBlocks {
            dense_line_index: dense_line_index.as_ref(),
            document_index,
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
    optional: &OptionalBlocks<'_>,
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
        usize_to_u64(input.len())?,
        Checksum::blake3(input),
        newline_mode_as_str(plan.newline_mode),
        plan.line_count,
        MetadataOptions {
            zstd_level: optional.writer_options.zstd_level,
            target_chunk_size: usize_to_u64(optional.writer_options.chunker.target_chunk_size)?,
            max_chunk_size: usize_to_u64(optional.writer_options.chunker.max_chunk_size)?,
            dictionary_mode: "none",
            profile: optional.profile,
            dense_line_index: optional.dense_line_index.is_some(),
            document_index: optional.document_index.is_some(),
        },
    );
    let metadata_bytes = metadata.encode()?;
    let metadata_size = usize_to_u64(metadata_bytes.len())?;

    let dense_line_index_bytes = optional
        .dense_line_index
        .map(DenseLineIndex::encode)
        .transpose()?;
    let dense_line_index_offset = metadata_offset
        .checked_add(metadata_size)
        .ok_or(QztError::PhysicalRangeOutOfBounds)?;
    let dense_line_index_size = dense_line_index_bytes
        .as_ref()
        .map(|bytes| usize_to_u64(bytes.len()))
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
        .map(|bytes| usize_to_u64(bytes.len()))
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
    let chunk_table_size = usize_to_u64(chunk_table_bytes.len())?;

    let mut blocks = vec![BlockDescriptor::chunk_table(
        chunk_table_offset,
        chunk_table_size,
        Checksum::blake3(&chunk_table_bytes),
    )];
    if let Some(bytes) = &dense_line_index_bytes {
        blocks.push(BlockDescriptor::dense_line_index(
            dense_line_index_offset,
            usize_to_u64(bytes.len())?,
            Checksum::blake3(bytes),
        ));
    }
    if let Some(bytes) = &document_index_bytes {
        blocks.push(BlockDescriptor::document_index(
            document_index_offset,
            usize_to_u64(bytes.len())?,
            Checksum::blake3(bytes),
        ));
    }

    let index_root = IndexRoot {
        container_id,
        blocks,
        original_size: metadata.original_size,
        original_checksum: metadata.original_checksum.clone(),
        chunk_count: usize_to_u64(entries.len())?,
        line_count: metadata.line_count,
    };
    let index_root_bytes = index_root.encode()?;
    let index_root_offset = chunk_table_offset
        .checked_add(chunk_table_size)
        .ok_or(QztError::PhysicalRangeOutOfBounds)?;
    let index_root_size = usize_to_u64(index_root_bytes.len())?;
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

    let index_root_ref = BlockRef {
        offset: index_root_offset,
        size: index_root_size,
        checksum: Checksum::blake3(&index_root_bytes),
    };
    let metadata_ref = BlockRef {
        offset: metadata_offset,
        size: metadata_size,
        checksum: Checksum::blake3(&metadata_bytes),
    };
    let prefix_checksum = Checksum::blake3(&prefix);
    let footer_payload = fixed_point_footer_payload(
        container_id,
        &index_root_ref,
        &metadata_ref,
        footer_payload_offset,
        Some(&prefix_checksum),
    )?;
    let footer_payload_bytes = footer_payload.encode()?;
    let footer_trailer = FooterTrailer {
        footer_payload_offset,
        footer_payload_size: usize_to_u64(footer_payload_bytes.len())?,
        footer_payload_checksum_blake3: Checksum::blake3(&footer_payload_bytes).value,
    };

    let mut bytes = prefix;
    bytes.extend_from_slice(&footer_payload_bytes);
    bytes.extend_from_slice(&footer_trailer.encode());
    Ok(bytes)
}

fn fixed_point_footer_payload(
    container_id: [u8; 16],
    index_root: &BlockRef,
    metadata: &BlockRef,
    footer_payload_offset: u64,
    container_checksum: Option<&Checksum>,
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
            container_checksum: container_checksum.cloned(),
        };
        let size = usize_to_u64(candidate.encode()?.len())?;
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

/// Validates that `profile` is one of the values defined by the QZT v0.1 spec.
///
/// Accepted values: `"minimal"`, `"core"`, `"log"`, `"archive"`, `"memory"`.
///
/// Returns [`QztError::MetadataInvalid`] for any other string.
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
