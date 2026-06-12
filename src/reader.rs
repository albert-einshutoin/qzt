use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::chunk_table::{ChunkEntry, STARTS_WITH_LINE_CONTINUATION};
use crate::error::{QztError, Result};
use crate::fixed::PhysicalRange;
use crate::format::FOOTER_TRAILER_LEN;
use crate::io::ReadAt;
use crate::limits::ResourceLimits;
use crate::primitives::{checked_logical_end, checked_physical_end, u64_to_usize, usize_to_u64};
use crate::schema::{Checksum, DictionaryEntry, DocumentEntry};
use crate::skeleton::{
    open_skeleton_details, open_skeleton_details_read_at, open_skeleton_details_with_limits,
    SkeletonDetails,
};

/// Reader for an in-memory QZT container.
pub struct QztReader {
    bytes: Vec<u8>,
    details: SkeletonDetails,
}

/// Reader for a positioned QZT source.
pub struct QztFileReader<R> {
    source: R,
    len: u64,
    details: SkeletonDetails,
}

/// Reader-visible container summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QztInfo {
    pub container_id: [u8; 16],
    pub original_size: u64,
    pub chunk_count: u64,
    pub line_count: u64,
}

/// Verification level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VerifyLevel {
    Quick,
    Normal,
    Deep,
}

/// Verification result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    pub level: VerifyLevel,
    pub checked_chunks: u64,
    pub decoded_bytes: u64,
}

impl QztReader {
    /// Opens an in-memory QZT container and performs quick structural validation.
    pub fn open(bytes: impl AsRef<[u8]>) -> Result<Self> {
        let bytes = bytes.as_ref().to_vec();
        let details = open_skeleton_details(&bytes)?;
        Ok(Self { bytes, details })
    }

    /// Opens an in-memory QZT container with explicit resource limits.
    pub fn open_with_limits(bytes: impl AsRef<[u8]>, limits: ResourceLimits) -> Result<Self> {
        let bytes = bytes.as_ref().to_vec();
        let details = open_skeleton_details_with_limits(&bytes, limits)?;
        Ok(Self { bytes, details })
    }

    pub fn info(&self) -> QztInfo {
        QztInfo {
            container_id: self.details.summary.container_id,
            original_size: self.details.summary.original_size,
            chunk_count: self.details.summary.chunk_count,
            line_count: self.details.summary.line_count,
        }
    }

    pub fn export_to<W: Write>(&self, mut writer: W) -> Result<()> {
        for entry in &self.details.chunk_entries {
            let decoded = self.decode_entry(entry)?;
            writer
                .write_all(&decoded)
                .map_err(|error| QztError::Io(error.kind()))?;
        }
        Ok(())
    }

    pub fn export_all(&self) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        self.export_to(&mut output)?;
        Ok(output)
    }

    pub fn read_range(&self, offset: u64, length: u64) -> Result<Vec<u8>> {
        read_range_from_entries(
            &self.details.chunk_entries,
            self.details.summary.original_size,
            offset,
            length,
            |entry| self.decode_entry(entry),
        )
    }

    pub fn read_text_range(&self, offset: u64, length: u64) -> Result<String> {
        let bytes = self.read_range(offset, length)?;
        String::from_utf8(bytes).map_err(|_| QztError::InvalidUtf8Boundary)
    }

    pub fn read_range_verified(
        &self,
        offset: u64,
        length: u64,
        expected: &Checksum,
    ) -> Result<Vec<u8>> {
        let bytes = self.read_range(offset, length)?;
        verify_expected_checksum(&bytes, expected)?;
        Ok(bytes)
    }

    pub fn read_document(&self, doc_id: &str) -> Result<Vec<u8>> {
        let document = find_document(&self.details, doc_id)?;
        self.read_range(document.logical_offset, document.byte_length)
    }

    pub fn read_document_verified(&self, doc_id: &str, expected: &Checksum) -> Result<Vec<u8>> {
        let bytes = self.read_document(doc_id)?;
        verify_expected_checksum(&bytes, expected)?;
        Ok(bytes)
    }

    pub fn read_line_raw(&self, line_zero_based: u64) -> Result<Vec<u8>> {
        read_line_from_entries(&self.details, line_zero_based, |entry| {
            self.decode_entry(entry)
        })
    }

    pub fn read_line_text(&self, line_zero_based: u64) -> Result<String> {
        String::from_utf8(self.read_line_raw(line_zero_based)?).map_err(|_| QztError::InvalidUtf8)
    }

    pub fn verify(&self, level: VerifyLevel) -> Result<VerifyReport> {
        match level {
            VerifyLevel::Quick => Ok(VerifyReport {
                level,
                checked_chunks: self.details.summary.chunk_count,
                decoded_bytes: 0,
            }),
            VerifyLevel::Normal => self.verify_normal(),
            VerifyLevel::Deep => self.verify_deep(),
        }
    }

    fn verify_normal(&self) -> Result<VerifyReport> {
        for entry in &self.details.chunk_entries {
            let compressed = self.slice_physical(PhysicalRange::new(
                entry.physical_offset,
                entry.compressed_size,
            ))?;
            if Checksum::blake3(compressed).value != entry.compressed_checksum_blake3 {
                return Err(QztError::CompressedChunkChecksumMismatch);
            }
        }

        if let Some(expected) = &self.details.footer_payload.container_checksum {
            let end = usize::try_from(self.details.footer_payload_offset)
                .map_err(|_| QztError::PhysicalRangeOutOfBounds)?;
            let prefix = self
                .bytes
                .get(..end)
                .ok_or(QztError::PhysicalRangeOutOfBounds)?;
            if Checksum::blake3(prefix) != *expected {
                return Err(QztError::ContainerCorrupt);
            }
        }

        Ok(VerifyReport {
            level: VerifyLevel::Normal,
            checked_chunks: self.details.summary.chunk_count,
            decoded_bytes: 0,
        })
    }

    fn verify_deep(&self) -> Result<VerifyReport> {
        self.verify_normal()?;
        verify_deep_entries(&self.details, |entry| self.decode_entry(entry))
    }

    /// Range read that reuses `cache` across calls so consecutive reads in the
    /// same chunk decode it only once. Used by search hit verification.
    pub(crate) fn read_range_cached(
        &self,
        offset: u64,
        length: u64,
        cache: &mut ChunkDecodeCache,
    ) -> Result<Vec<u8>> {
        read_range_from_entries_cached(
            &self.details.chunk_entries,
            self.details.summary.original_size,
            offset,
            length,
            cache,
            |entry| self.decode_entry(entry),
        )
    }

    fn decode_entry(&self, entry: &ChunkEntry) -> Result<Vec<u8>> {
        let compressed = self.slice_physical(PhysicalRange::new(
            entry.physical_offset,
            entry.compressed_size,
        ))?;
        decode_compressed_entry(entry, compressed, &self.details.dictionaries)
    }

    fn slice_physical(&self, range: PhysicalRange) -> Result<&[u8]> {
        let end = checked_physical_end(range.offset, range.size)?;
        if end > self.bytes.len() as u64 {
            return Err(QztError::PhysicalRangeOutOfBounds);
        }
        let start =
            usize::try_from(range.offset).map_err(|_| QztError::PhysicalRangeOutOfBounds)?;
        let end = usize::try_from(end).map_err(|_| QztError::PhysicalRangeOutOfBounds)?;
        self.bytes
            .get(start..end)
            .ok_or(QztError::PhysicalRangeOutOfBounds)
    }
}

impl<R: ReadAt> QztFileReader<R> {
    /// Opens a QZT reader over a positioned source with default resource limits.
    pub fn open_read_at(source: R, len: u64) -> Result<Self> {
        Self::open_read_at_with_limits(source, len, ResourceLimits::default())
    }

    /// Opens a QZT reader over a positioned source with explicit resource limits.
    pub fn open_read_at_with_limits(source: R, len: u64, limits: ResourceLimits) -> Result<Self> {
        let details = open_skeleton_details_read_at(&source, len, limits)?;
        Ok(Self {
            source,
            len,
            details,
        })
    }

    /// Returns the wrapped positioned source.
    pub fn into_inner(self) -> R {
        self.source
    }

    pub fn info(&self) -> QztInfo {
        QztInfo {
            container_id: self.details.summary.container_id,
            original_size: self.details.summary.original_size,
            chunk_count: self.details.summary.chunk_count,
            line_count: self.details.summary.line_count,
        }
    }

    pub fn export_to<W: Write>(&self, mut writer: W) -> Result<()> {
        for entry in &self.details.chunk_entries {
            let decoded = self.decode_entry(entry)?;
            writer
                .write_all(&decoded)
                .map_err(|error| QztError::Io(error.kind()))?;
        }
        Ok(())
    }

    pub fn export_all(&self) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        self.export_to(&mut output)?;
        Ok(output)
    }

    pub fn read_range(&self, offset: u64, length: u64) -> Result<Vec<u8>> {
        read_range_from_entries(
            &self.details.chunk_entries,
            self.details.summary.original_size,
            offset,
            length,
            |entry| self.decode_entry(entry),
        )
    }

    pub fn read_text_range(&self, offset: u64, length: u64) -> Result<String> {
        let bytes = self.read_range(offset, length)?;
        String::from_utf8(bytes).map_err(|_| QztError::InvalidUtf8Boundary)
    }

    pub fn read_range_verified(
        &self,
        offset: u64,
        length: u64,
        expected: &Checksum,
    ) -> Result<Vec<u8>> {
        let bytes = self.read_range(offset, length)?;
        verify_expected_checksum(&bytes, expected)?;
        Ok(bytes)
    }

    pub fn read_document(&self, doc_id: &str) -> Result<Vec<u8>> {
        let document = find_document(&self.details, doc_id)?;
        self.read_range(document.logical_offset, document.byte_length)
    }

    pub fn read_document_verified(&self, doc_id: &str, expected: &Checksum) -> Result<Vec<u8>> {
        let bytes = self.read_document(doc_id)?;
        verify_expected_checksum(&bytes, expected)?;
        Ok(bytes)
    }

    pub fn read_line_raw(&self, line_zero_based: u64) -> Result<Vec<u8>> {
        read_line_from_entries(&self.details, line_zero_based, |entry| {
            self.decode_entry(entry)
        })
    }

    pub fn read_line_text(&self, line_zero_based: u64) -> Result<String> {
        String::from_utf8(self.read_line_raw(line_zero_based)?).map_err(|_| QztError::InvalidUtf8)
    }

    pub fn verify(&self, level: VerifyLevel) -> Result<VerifyReport> {
        match level {
            VerifyLevel::Quick => Ok(VerifyReport {
                level,
                checked_chunks: self.details.summary.chunk_count,
                decoded_bytes: 0,
            }),
            VerifyLevel::Normal => self.verify_normal(),
            VerifyLevel::Deep => self.verify_deep(),
        }
    }

    fn verify_normal(&self) -> Result<VerifyReport> {
        for entry in &self.details.chunk_entries {
            let compressed = self.read_physical(PhysicalRange::new(
                entry.physical_offset,
                entry.compressed_size,
            ))?;
            if Checksum::blake3(&compressed).value != entry.compressed_checksum_blake3 {
                return Err(QztError::CompressedChunkChecksumMismatch);
            }
        }

        if let Some(expected) = &self.details.footer_payload.container_checksum {
            let actual = self.hash_physical_prefix(self.details.footer_payload_offset)?;
            if actual != *expected {
                return Err(QztError::ContainerCorrupt);
            }
        }

        Ok(VerifyReport {
            level: VerifyLevel::Normal,
            checked_chunks: self.details.summary.chunk_count,
            decoded_bytes: 0,
        })
    }

    fn verify_deep(&self) -> Result<VerifyReport> {
        self.verify_normal()?;
        verify_deep_entries(&self.details, |entry| self.decode_entry(entry))
    }

    /// Range read that reuses `cache` across calls so consecutive reads in the
    /// same chunk decode it only once. Used by search hit verification.
    pub(crate) fn read_range_cached(
        &self,
        offset: u64,
        length: u64,
        cache: &mut ChunkDecodeCache,
    ) -> Result<Vec<u8>> {
        read_range_from_entries_cached(
            &self.details.chunk_entries,
            self.details.summary.original_size,
            offset,
            length,
            cache,
            |entry| self.decode_entry(entry),
        )
    }

    /// Parsed structural details (metadata, chunk table, indexes).
    ///
    /// Exposed for the CLI and search wiring; not part of the documented
    /// stable surface.
    #[doc(hidden)]
    pub fn skeleton_details(&self) -> &SkeletonDetails {
        &self.details
    }

    /// BLAKE3 checksum of the footer payload region, streamed with a bounded
    /// buffer. Used to bind search sidecars to this exact container.
    pub(crate) fn footer_checksum(&self) -> Result<Checksum> {
        let end = self
            .len
            .checked_sub(FOOTER_TRAILER_LEN as u64)
            .ok_or(QztError::InvalidFooterTrailer)?;
        self.hash_physical_range(self.details.footer_payload_offset, end)
    }

    pub(crate) fn decode_entry(&self, entry: &ChunkEntry) -> Result<Vec<u8>> {
        let compressed = self.read_physical(PhysicalRange::new(
            entry.physical_offset,
            entry.compressed_size,
        ))?;
        decode_compressed_entry(entry, &compressed, &self.details.dictionaries)
    }

    fn read_physical(&self, range: PhysicalRange) -> Result<Vec<u8>> {
        let end = checked_physical_end(range.offset, range.size)?;
        if end > self.len {
            return Err(QztError::PhysicalRangeOutOfBounds);
        }
        let len = u64_to_usize(range.size)?;
        let mut bytes = vec![0_u8; len];
        self.source
            .read_exact_at(range.offset, &mut bytes)
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::UnexpectedEof => QztError::UnexpectedEof,
                _ => QztError::ContainerCorrupt,
            })?;
        Ok(bytes)
    }

    fn hash_physical_prefix(&self, end: u64) -> Result<Checksum> {
        self.hash_physical_range(0, end)
    }

    fn hash_physical_range(&self, start: u64, end: u64) -> Result<Checksum> {
        if end > self.len || start > end {
            return Err(QztError::PhysicalRangeOutOfBounds);
        }
        let mut hasher = blake3::Hasher::new();
        let mut offset = start;
        let mut buffer = vec![0_u8; 64 * 1024];
        while offset < end {
            let remaining = end - offset;
            let read_len = usize::try_from(remaining.min(buffer.len() as u64))
                .map_err(|_| QztError::ResourceLimitExceeded)?;
            self.source
                .read_exact_at(offset, &mut buffer[..read_len])
                .map_err(|error| match error.kind() {
                    std::io::ErrorKind::UnexpectedEof => QztError::UnexpectedEof,
                    _ => QztError::ContainerCorrupt,
                })?;
            hasher.update(&buffer[..read_len]);
            offset = offset
                .checked_add(read_len as u64)
                .ok_or(QztError::PhysicalRangeOutOfBounds)?;
        }
        Ok(Checksum::from_hasher(&hasher))
    }
}

impl QztFileReader<File> {
    /// Opens a QZT file from a filesystem path.
    pub fn open_path(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path).map_err(|error| QztError::Io(error.kind()))?;
        let len = file
            .metadata()
            .map_err(|error| QztError::Io(error.kind()))?
            .len();
        Self::open_read_at(file, len)
    }
}

/// Single-entry cache of the most recently decoded chunk.
///
/// Range reads decode whole chunks so the per-chunk checksums can be verified
/// before any byte is returned. Callers that issue many small range reads in
/// ascending order (for example search hit verification over sorted granules)
/// reuse one cache across calls so each chunk is decoded at most once instead
/// of once per read.
pub(crate) struct ChunkDecodeCache {
    chunk_id: Option<u64>,
    decoded: Vec<u8>,
    physical_decoded_bytes: u64,
}

impl ChunkDecodeCache {
    pub(crate) fn new() -> Self {
        Self {
            chunk_id: None,
            decoded: Vec::new(),
            physical_decoded_bytes: 0,
        }
    }

    /// Total uncompressed bytes decoded through this cache (cache misses only).
    pub(crate) fn physical_decoded_bytes(&self) -> u64 {
        self.physical_decoded_bytes
    }

    fn decoded_entry(
        &mut self,
        entry: &ChunkEntry,
        decode_entry: &mut impl FnMut(&ChunkEntry) -> Result<Vec<u8>>,
    ) -> Result<&[u8]> {
        if self.chunk_id != Some(entry.chunk_id) {
            self.decoded = decode_entry(entry)?;
            self.chunk_id = Some(entry.chunk_id);
            self.physical_decoded_bytes = self
                .physical_decoded_bytes
                .checked_add(entry.uncompressed_size)
                .ok_or(QztError::ResourceLimitExceeded)?;
        }
        Ok(&self.decoded)
    }
}

fn read_range_from_entries(
    entries: &[ChunkEntry],
    original_size: u64,
    offset: u64,
    length: u64,
    decode_entry: impl FnMut(&ChunkEntry) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    let mut cache = ChunkDecodeCache::new();
    read_range_from_entries_cached(
        entries,
        original_size,
        offset,
        length,
        &mut cache,
        decode_entry,
    )
}

fn read_range_from_entries_cached(
    entries: &[ChunkEntry],
    original_size: u64,
    offset: u64,
    length: u64,
    cache: &mut ChunkDecodeCache,
    mut decode_entry: impl FnMut(&ChunkEntry) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    let end = checked_logical_end(offset, length)?;
    if end > original_size {
        return Err(QztError::LogicalRangeOutOfBounds);
    }
    if length == 0 {
        return Ok(Vec::new());
    }

    let mut output = Vec::new();
    let mut index = range_start_chunk_index(entries, offset)?;
    while let Some(entry) = entries.get(index) {
        let chunk_end = checked_logical_end(entry.logical_offset, entry.uncompressed_size)?;
        if entry.logical_offset >= end {
            break;
        }

        let decoded = cache.decoded_entry(entry, &mut decode_entry)?;
        let copy_start = offset.max(entry.logical_offset);
        let copy_end = end.min(chunk_end);
        let local_start = usize::try_from(copy_start - entry.logical_offset)
            .map_err(|_| QztError::ResourceLimitExceeded)?;
        let local_end = usize::try_from(copy_end - entry.logical_offset)
            .map_err(|_| QztError::ResourceLimitExceeded)?;
        output.extend_from_slice(&decoded[local_start..local_end]);
        index += 1;
    }

    if usize_to_u64(output.len())? != length {
        return Err(QztError::ContainerCorrupt);
    }

    Ok(output)
}

fn read_line_from_entries(
    details: &SkeletonDetails,
    line_zero_based: u64,
    mut decode_entry: impl FnMut(&ChunkEntry) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    if line_zero_based >= details.summary.line_count {
        return Err(QztError::LineOutOfRange);
    }

    let start_index = line_start_chunk_index(&details.chunk_entries, line_zero_based)?;
    let start_entry = details
        .chunk_entries
        .get(start_index)
        .ok_or(QztError::LineOutOfRange)?;
    let start_decoded = decode_entry(start_entry)?;
    let local_index = usize::try_from(line_zero_based - start_entry.first_line)
        .map_err(|_| QztError::LineOutOfRange)?;
    let local_start = if let Some(dense) = &details.dense_line_index {
        usize::try_from(dense.line_start_offset(start_index, local_index)?)
            .map_err(|_| QztError::ResourceLimitExceeded)?
    } else {
        let starts = local_line_starts(&start_decoded, start_entry.flags);
        starts
            .get(local_index)
            .copied()
            .ok_or(QztError::LineOutOfRange)?
    };

    let mut output = Vec::new();
    if append_until_lf(&start_decoded, local_start, &mut output) {
        return Ok(output);
    }

    let mut current_index = start_index + 1;
    while let Some(entry) = details.chunk_entries.get(current_index) {
        let decoded = decode_entry(entry)?;
        let found_end = append_until_lf(&decoded, 0, &mut output);
        if found_end {
            return Ok(output);
        }
        current_index += 1;
    }

    Ok(output)
}

fn verify_deep_entries(
    details: &SkeletonDetails,
    mut decode_entry: impl FnMut(&ChunkEntry) -> Result<Vec<u8>>,
) -> Result<VerifyReport> {
    let mut original_hasher = blake3::Hasher::new();
    let mut text = StreamingTextAnalysis::new();
    let mut document_hasher = details.document_index.as_ref().map(DocumentHasher::new);
    let mut decoded_bytes = 0_u64;

    for (chunk_index, entry) in details.chunk_entries.iter().enumerate() {
        let expected_flags = if entry.logical_offset > 0 && text.previous_byte != Some(b'\n') {
            STARTS_WITH_LINE_CONTINUATION
        } else {
            0
        };
        if entry.flags != expected_flags {
            return Err(QztError::ChunkTableInvalid);
        }
        if entry.first_line != text.line_starts_seen {
            return Err(QztError::ChunkTableInvalid);
        }

        let decoded = decode_entry(entry)?;
        std::str::from_utf8(&decoded).map_err(|_| QztError::InvalidUtf8)?;
        if let Some(dense) = &details.dense_line_index {
            dense.verify_chunk(chunk_index, &decoded, entry.flags)?;
        }

        let chunk_line_count = u64::try_from(local_line_starts(&decoded, entry.flags).len())
            .map_err(|_| QztError::ResourceLimitExceeded)?;
        if entry.line_count != chunk_line_count {
            return Err(QztError::ChunkTableInvalid);
        }

        original_hasher.update(&decoded);
        text.update(&decoded, entry.flags)?;
        if let Some(hasher) = document_hasher.as_mut() {
            hasher.feed(entry.logical_offset, &decoded)?;
        }
        decoded_bytes = decoded_bytes
            .checked_add(usize_to_u64(decoded.len())?)
            .ok_or(QztError::ResourceLimitExceeded)?;
    }

    if decoded_bytes != details.summary.original_size {
        return Err(QztError::ChunkSizeMismatch);
    }
    let original_checksum = Checksum::from_hasher(&original_hasher);
    if original_checksum != details.metadata.original_checksum {
        return Err(QztError::UncompressedChunkChecksumMismatch);
    }
    if text.line_starts_seen != details.metadata.line_count {
        return Err(QztError::ContainerCorrupt);
    }
    if text.newline_mode() != details.metadata.newline_mode {
        return Err(QztError::NewlineModeMismatch);
    }

    if let Some(document_index) = &details.document_index {
        let document_hashes = document_hasher
            .map(DocumentHasher::finish)
            .unwrap_or_default();
        verify_document_index_ranges(
            document_index,
            details.summary.original_size,
            details.metadata.line_count,
            &details.chunk_entries,
            &document_hashes,
        )?;
    }

    Ok(VerifyReport {
        level: VerifyLevel::Deep,
        checked_chunks: details.summary.chunk_count,
        decoded_bytes,
    })
}

fn decode_compressed_entry(
    entry: &ChunkEntry,
    compressed: &[u8],
    dictionaries: &[DictionaryEntry],
) -> Result<Vec<u8>> {
    if Checksum::blake3(compressed).value != entry.compressed_checksum_blake3 {
        return Err(QztError::CompressedChunkChecksumMismatch);
    }

    let dictionary = if entry.dictionary_id == 0 {
        &[][..]
    } else {
        dictionaries
            .iter()
            .find(|dictionary| dictionary.dictionary_id == entry.dictionary_id)
            .map(|dictionary| dictionary.bytes.as_slice())
            .ok_or(QztError::MissingDictionary)?
    };
    let decoder = zstd::stream::Decoder::with_dictionary(compressed, dictionary)
        .map_err(|_| QztError::ZstdDecodeError)?;
    let decoded = decode_with_output_limit(decoder, entry.uncompressed_size)?;
    if usize_to_u64(decoded.len())? != entry.uncompressed_size {
        return Err(QztError::ChunkSizeMismatch);
    }
    if Checksum::blake3(&decoded).value != entry.uncompressed_checksum_blake3 {
        return Err(QztError::UncompressedChunkChecksumMismatch);
    }
    Ok(decoded)
}

fn find_document<'a>(details: &'a SkeletonDetails, doc_id: &str) -> Result<&'a DocumentEntry> {
    let document_index = details
        .document_index
        .as_ref()
        .ok_or(QztError::MissingRequiredBlock)?;
    let index = *details
        .document_lookup
        .get(doc_id)
        .ok_or(QztError::DocumentNotFound)?;
    document_index
        .documents
        .get(index)
        .ok_or(QztError::DocumentNotFound)
}

fn verify_expected_checksum(bytes: &[u8], expected: &Checksum) -> Result<()> {
    if expected.algorithm != crate::schema::CHECKSUM_ALGORITHM_BLAKE3 {
        return Err(QztError::ContainerCorrupt);
    }
    if Checksum::blake3(bytes) != *expected {
        return Err(QztError::VerifiedChecksumMismatch);
    }
    Ok(())
}

struct StreamingTextAnalysis {
    line_starts_seen: u64,
    lf_count: u64,
    crlf_count: u64,
    previous_byte: Option<u8>,
}

impl StreamingTextAnalysis {
    fn new() -> Self {
        Self {
            line_starts_seen: 0,
            lf_count: 0,
            crlf_count: 0,
            previous_byte: None,
        }
    }

    fn update(&mut self, decoded: &[u8], flags: u32) -> Result<()> {
        let starts = local_line_starts(decoded, flags);
        self.line_starts_seen = self
            .line_starts_seen
            .checked_add(usize_to_u64(starts.len())?)
            .ok_or(QztError::ResourceLimitExceeded)?;

        for byte in decoded {
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

    fn newline_mode(&self) -> String {
        match (self.lf_count > 0, self.crlf_count > 0) {
            (false, false) => "none",
            (true, false) => "lf",
            (false, true) => "crlf",
            (true, true) => "mixed",
        }
        .to_owned()
    }
}

fn range_start_chunk_index(entries: &[ChunkEntry], offset: u64) -> Result<usize> {
    let mut low = 0_usize;
    let mut high = entries.len();

    while low < high {
        let mid = low + (high - low) / 2;
        let chunk_end =
            checked_logical_end(entries[mid].logical_offset, entries[mid].uncompressed_size)?;
        if chunk_end <= offset {
            low = mid + 1;
        } else {
            high = mid;
        }
    }

    Ok(low)
}

fn line_start_chunk_index(entries: &[ChunkEntry], line_zero_based: u64) -> Result<usize> {
    let mut low = 0_usize;
    let mut high = entries.len();

    while low < high {
        let mid = low + (high - low) / 2;
        let line_end = checked_logical_end(entries[mid].first_line, entries[mid].line_count)?;
        if line_end <= line_zero_based {
            low = mid + 1;
        } else {
            high = mid;
        }
    }

    let entry = entries.get(low).ok_or(QztError::LineOutOfRange)?;
    let line_end = checked_logical_end(entry.first_line, entry.line_count)?;
    if entry.first_line <= line_zero_based && line_zero_based < line_end {
        Ok(low)
    } else {
        Err(QztError::LineOutOfRange)
    }
}

fn decode_with_output_limit(
    decoder: zstd::stream::Decoder<'_, &[u8]>,
    expected_size: u64,
) -> Result<Vec<u8>> {
    let capacity = u64_to_usize(expected_size)?;
    let read_limit = expected_size
        .checked_add(1)
        .ok_or(QztError::ResourceLimitExceeded)?;
    let mut decoded = Vec::with_capacity(capacity);
    let mut limited = decoder.take(read_limit);
    limited
        .read_to_end(&mut decoded)
        .map_err(|_| QztError::ZstdDecodeError)?;

    if usize_to_u64(decoded.len())? > expected_size {
        return Err(QztError::ResourceLimitExceeded);
    }

    Ok(decoded)
}

fn local_line_starts(decoded: &[u8], flags: u32) -> Vec<usize> {
    let mut starts = Vec::new();
    if flags & STARTS_WITH_LINE_CONTINUATION == 0 && !decoded.is_empty() {
        starts.push(0);
    }

    for index in 0..decoded.len() {
        if decoded[index] == b'\n' && index + 1 < decoded.len() {
            starts.push(index + 1);
        }
    }

    starts
}

fn append_until_lf(decoded: &[u8], start: usize, output: &mut Vec<u8>) -> bool {
    for byte in &decoded[start..] {
        output.push(*byte);
        if *byte == b'\n' {
            return true;
        }
    }
    false
}

fn verify_document_index_ranges(
    document_index: &crate::schema::DocumentIndex,
    original_size: u64,
    line_count: u64,
    chunk_entries: &[ChunkEntry],
    document_hashes: &HashMap<usize, [u8; 32]>,
) -> Result<()> {
    for (index, document) in document_index.documents.iter().enumerate() {
        let end = checked_logical_end(document.logical_offset, document.byte_length)?;
        if end > original_size {
            return Err(QztError::LogicalRangeOutOfBounds);
        }
        let line_end = checked_logical_end(document.first_line, document.line_count)?;
        if line_end > line_count {
            return Err(QztError::LineOutOfRange);
        }

        let mut expected_doc_hash = [0_u8; 16];
        let doc_hash = blake3::hash(document.doc_id.as_bytes());
        expected_doc_hash.copy_from_slice(&doc_hash.as_bytes()[..16]);
        if document.doc_id_hash != expected_doc_hash {
            return Err(QztError::ContainerCorrupt);
        }

        // Non-empty documents are hashed in a single pass during the deep-verify
        // chunk loop; empty documents need no decoded bytes.
        let actual = if document.byte_length == 0 {
            Checksum::blake3(&[])
        } else {
            let value = document_hashes
                .get(&index)
                .copied()
                .ok_or(QztError::ContainerCorrupt)?;
            Checksum::from_raw_bytes(value)
        };
        if actual != document.checksum {
            return Err(QztError::ContainerCorrupt);
        }

        let (chunk_start, chunk_end) =
            document_chunk_range(chunk_entries, document.logical_offset, document.byte_length)?;
        if document.chunk_start != chunk_start || document.chunk_end != chunk_end {
            return Err(QztError::ChunkTableInvalid);
        }
    }

    Ok(())
}

/// Chunk span `[chunk_start, chunk_end)` covering a logical document range.
///
/// Chunks are contiguous and ordered by logical offset, so the bounds are found
/// with two binary searches instead of an O(chunks) scan per document.
fn document_chunk_range(
    chunk_entries: &[ChunkEntry],
    offset: u64,
    length: u64,
) -> Result<(u64, u64)> {
    if length == 0 {
        return Ok((0, 0));
    }
    let end = checked_logical_end(offset, length)?;

    let first_index = range_start_chunk_index(chunk_entries, offset)?;
    let last_index = range_start_chunk_index(chunk_entries, end - 1)?;
    let first = chunk_entries
        .get(first_index)
        .ok_or(QztError::ChunkTableInvalid)?
        .chunk_id;
    let last = chunk_entries
        .get(last_index)
        .ok_or(QztError::ChunkTableInvalid)?
        .chunk_id;
    let last_exclusive = last.checked_add(1).ok_or(QztError::ChunkTableInvalid)?;
    Ok((first, last_exclusive))
}

/// Single-pass BLAKE3 hasher for document ranges, fed decoded chunks in logical
/// order during deep verify so document checksums never trigger a re-decode.
///
/// Documents are caller-supplied and may appear in any order or overlap, so the
/// non-empty entries are sorted by logical offset and activated as the covering
/// chunks arrive. Empty documents are verified separately without decoded bytes.
struct DocumentHasher {
    pending: Vec<PendingDocument>,
    next: usize,
    active: Vec<ActiveDocument>,
    results: HashMap<usize, [u8; 32]>,
}

struct PendingDocument {
    index: usize,
    start: u64,
    end: u64,
}

struct ActiveDocument {
    index: usize,
    start: u64,
    end: u64,
    hasher: blake3::Hasher,
}

impl DocumentHasher {
    fn new(document_index: &crate::schema::DocumentIndex) -> Self {
        let mut pending: Vec<PendingDocument> = document_index
            .documents
            .iter()
            .enumerate()
            .filter(|(_, document)| document.byte_length > 0)
            .map(|(index, document)| PendingDocument {
                index,
                start: document.logical_offset,
                end: document.logical_offset.saturating_add(document.byte_length),
            })
            .collect();
        pending.sort_by(|a, b| a.start.cmp(&b.start).then(a.index.cmp(&b.index)));
        Self {
            pending,
            next: 0,
            active: Vec::new(),
            results: HashMap::new(),
        }
    }

    fn feed(&mut self, chunk_offset: u64, decoded: &[u8]) -> Result<()> {
        let chunk_len = usize_to_u64(decoded.len())?;
        let chunk_end = chunk_offset
            .checked_add(chunk_len)
            .ok_or(QztError::ResourceLimitExceeded)?;

        while let Some(pending) = self.pending.get(self.next) {
            if pending.start >= chunk_end {
                break;
            }
            self.active.push(ActiveDocument {
                index: pending.index,
                start: pending.start,
                end: pending.end,
                hasher: blake3::Hasher::new(),
            });
            self.next += 1;
        }

        let mut still_active = Vec::with_capacity(self.active.len());
        for mut document in self.active.drain(..) {
            let lower = chunk_offset.max(document.start);
            let upper = chunk_end.min(document.end);
            if lower < upper {
                let local_start = usize::try_from(lower - chunk_offset)
                    .map_err(|_| QztError::ResourceLimitExceeded)?;
                let local_end = usize::try_from(upper - chunk_offset)
                    .map_err(|_| QztError::ResourceLimitExceeded)?;
                let slice = decoded
                    .get(local_start..local_end)
                    .ok_or(QztError::ContainerCorrupt)?;
                document.hasher.update(slice);
            }
            if document.end <= chunk_end {
                self.results
                    .insert(document.index, *document.hasher.finalize().as_bytes());
            } else {
                still_active.push(document);
            }
        }
        self.active = still_active;

        Ok(())
    }

    fn finish(self) -> HashMap<usize, [u8; 32]> {
        self.results
    }
}

#[cfg(test)]
mod document_hasher_tests {
    use super::*;
    use crate::schema::{Checksum, DocumentEntry, DocumentIndex};

    fn entry(doc_id: &str, data: &[u8], offset: u64, length: u64) -> DocumentEntry {
        let start = u64_to_usize(offset).expect("offset fits in usize in tests");
        let end = start + u64_to_usize(length).expect("length fits in usize in tests");
        DocumentEntry::new(
            doc_id,
            offset,
            length,
            0,
            0,
            0,
            0,
            Checksum::blake3(&data[start..end]),
        )
    }

    fn feed_in_chunks(
        index: &DocumentIndex,
        data: &[u8],
        chunk: usize,
    ) -> HashMap<usize, [u8; 32]> {
        let mut hasher = DocumentHasher::new(index);
        let mut offset = 0_usize;
        while offset < data.len() {
            let end = (offset + chunk).min(data.len());
            hasher
                .feed(offset as u64, &data[offset..end])
                .expect("feed should succeed");
            offset = end;
        }
        hasher.finish()
    }

    fn expected(data: &[u8], offset: u64, length: u64) -> [u8; 32] {
        let start = u64_to_usize(offset).expect("offset fits in usize in tests");
        let end = u64_to_usize(offset + length).expect("offset+length fits in usize in tests");
        *blake3::hash(&data[start..end]).as_bytes()
    }

    #[test]
    fn hashes_document_contained_in_a_single_chunk() {
        let data = b"hello world!!!!!";
        let index = DocumentIndex {
            container_id: [0; 16],
            documents: vec![entry("a", data, 6, 5)],
        };
        let results = feed_in_chunks(&index, data, 16);
        assert_eq!(results.get(&0).copied(), Some(expected(data, 6, 5)));
    }

    #[test]
    fn hashes_document_spanning_multiple_chunks() {
        let data = b"abcdefghijklmnopqrstuvwxyz0123456789";
        let index = DocumentIndex {
            container_id: [0; 16],
            documents: vec![entry("a", data, 2, 30)],
        };
        // Feed in 4-byte chunks so the document crosses several boundaries.
        let results = feed_in_chunks(&index, data, 4);
        assert_eq!(results.get(&0).copied(), Some(expected(data, 2, 30)));
    }

    #[test]
    fn hashes_out_of_order_documents_by_their_original_index() {
        let data = b"abcdefghijklmnopqrstuvwxyz012345";
        // Listed with the later range first; results must key by document index.
        let index = DocumentIndex {
            container_id: [0; 16],
            documents: vec![entry("late", data, 20, 12), entry("early", data, 0, 8)],
        };
        let results = feed_in_chunks(&index, data, 5);
        assert_eq!(results.get(&0).copied(), Some(expected(data, 20, 12)));
        assert_eq!(results.get(&1).copied(), Some(expected(data, 0, 8)));
    }

    #[test]
    fn hashes_overlapping_documents_independently() {
        let data = b"abcdefghijklmnop";
        let index = DocumentIndex {
            container_id: [0; 16],
            documents: vec![entry("wide", data, 0, 16), entry("inner", data, 4, 6)],
        };
        let results = feed_in_chunks(&index, data, 3);
        assert_eq!(results.get(&0).copied(), Some(expected(data, 0, 16)));
        assert_eq!(results.get(&1).copied(), Some(expected(data, 4, 6)));
    }

    #[test]
    fn empty_documents_are_excluded_from_results() {
        let data = b"abcdefgh";
        let index = DocumentIndex {
            container_id: [0; 16],
            documents: vec![entry("empty", data, 4, 0), entry("real", data, 0, 4)],
        };
        let results = feed_in_chunks(&index, data, 8);
        assert!(!results.contains_key(&0));
        assert_eq!(results.get(&1).copied(), Some(expected(data, 0, 4)));
    }
}
