use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::chunk_table::{ChunkEntry, STARTS_WITH_LINE_CONTINUATION};
use crate::error::{QztError, Result};
use crate::fixed::PhysicalRange;
use crate::io::ReadAt;
use crate::limits::ResourceLimits;
use crate::primitives::{checked_logical_end, checked_physical_end};
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
                .map_err(|_| QztError::ContainerCorrupt)?;
        }
        Ok(())
    }

    pub fn export_all(&self) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        self.export_to(&mut output)?;
        Ok(output)
    }

    pub fn read_range(&self, offset: u64, length: u64) -> Result<Vec<u8>> {
        let end = checked_logical_end(offset, length)?;
        if end > self.details.summary.original_size {
            return Err(QztError::LogicalRangeOutOfBounds);
        }
        if length == 0 {
            return Ok(Vec::new());
        }

        let mut output = Vec::new();
        let mut index = range_start_chunk_index(&self.details.chunk_entries, offset)?;
        while let Some(entry) = self.details.chunk_entries.get(index) {
            let chunk_end = checked_logical_end(entry.logical_offset, entry.uncompressed_size)?;
            if entry.logical_offset >= end {
                break;
            }

            let decoded = self.decode_entry(entry)?;
            let copy_start = offset.max(entry.logical_offset);
            let copy_end = end.min(chunk_end);
            let local_start = usize::try_from(copy_start - entry.logical_offset)
                .map_err(|_| QztError::ResourceLimitExceeded)?;
            let local_end = usize::try_from(copy_end - entry.logical_offset)
                .map_err(|_| QztError::ResourceLimitExceeded)?;
            output.extend_from_slice(&decoded[local_start..local_end]);
            index += 1;
        }

        if u64::try_from(output.len()).map_err(|_| QztError::ResourceLimitExceeded)? != length {
            return Err(QztError::ContainerCorrupt);
        }

        Ok(output)
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
        if line_zero_based >= self.details.summary.line_count {
            return Err(QztError::LineOutOfRange);
        }

        let start_index = line_start_chunk_index(&self.details.chunk_entries, line_zero_based)?;

        let start_entry = self
            .details
            .chunk_entries
            .get(start_index)
            .ok_or(QztError::LineOutOfRange)?;
        let start_decoded = self.decode_entry(start_entry)?;
        let local_index = usize::try_from(line_zero_based - start_entry.first_line)
            .map_err(|_| QztError::LineOutOfRange)?;
        let local_start = if let Some(dense) = &self.details.dense_line_index {
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
        while let Some(entry) = self.details.chunk_entries.get(current_index) {
            let decoded = self.decode_entry(entry)?;
            let found_end = append_until_lf(&decoded, 0, &mut output);
            if found_end {
                return Ok(output);
            }
            current_index += 1;
        }

        Ok(output)
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
                .map_err(|_| QztError::ContainerCorrupt)?;
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

    fn decode_entry(&self, entry: &ChunkEntry) -> Result<Vec<u8>> {
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
        let len = usize::try_from(range.size).map_err(|_| QztError::ResourceLimitExceeded)?;
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
        if end > self.len {
            return Err(QztError::PhysicalRangeOutOfBounds);
        }
        let mut hasher = blake3::Hasher::new();
        let mut offset = 0_u64;
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
        Ok(Checksum {
            algorithm: "blake3".to_owned(),
            value: *hasher.finalize().as_bytes(),
        })
    }
}

impl QztFileReader<File> {
    /// Opens a QZT file from a filesystem path.
    pub fn open_path(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path).map_err(|_| QztError::ContainerCorrupt)?;
        let len = file
            .metadata()
            .map_err(|_| QztError::ContainerCorrupt)?
            .len();
        Self::open_read_at(file, len)
    }
}

fn read_range_from_entries(
    entries: &[ChunkEntry],
    original_size: u64,
    offset: u64,
    length: u64,
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

        let decoded = decode_entry(entry)?;
        let copy_start = offset.max(entry.logical_offset);
        let copy_end = end.min(chunk_end);
        let local_start = usize::try_from(copy_start - entry.logical_offset)
            .map_err(|_| QztError::ResourceLimitExceeded)?;
        let local_end = usize::try_from(copy_end - entry.logical_offset)
            .map_err(|_| QztError::ResourceLimitExceeded)?;
        output.extend_from_slice(&decoded[local_start..local_end]);
        index += 1;
    }

    if u64::try_from(output.len()).map_err(|_| QztError::ResourceLimitExceeded)? != length {
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
    let mut text = StreamingTextAnalysis::new(details.summary.original_size);
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
        decoded_bytes = decoded_bytes
            .checked_add(u64::try_from(decoded.len()).map_err(|_| QztError::ResourceLimitExceeded)?)
            .ok_or(QztError::ResourceLimitExceeded)?;
    }

    if decoded_bytes != details.summary.original_size {
        return Err(QztError::ChunkSizeMismatch);
    }
    let original_checksum = Checksum {
        algorithm: "blake3".to_owned(),
        value: *original_hasher.finalize().as_bytes(),
    };
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
        verify_document_index_ranges(
            document_index,
            details.metadata.line_count,
            &details.chunk_entries,
            |entry| decode_entry(entry),
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
    if u64::try_from(decoded.len()).map_err(|_| QztError::ResourceLimitExceeded)?
        != entry.uncompressed_size
    {
        return Err(QztError::ChunkSizeMismatch);
    }
    if Checksum::blake3(&decoded).value != entry.uncompressed_checksum_blake3 {
        return Err(QztError::UncompressedChunkChecksumMismatch);
    }
    Ok(decoded)
}

fn find_document<'a>(details: &'a SkeletonDetails, doc_id: &str) -> Result<&'a DocumentEntry> {
    details
        .document_index
        .as_ref()
        .ok_or(QztError::MissingRequiredBlock)?
        .documents
        .iter()
        .find(|document| document.doc_id == doc_id)
        .ok_or(QztError::MissingRequiredBlock)
}

fn verify_expected_checksum(bytes: &[u8], expected: &Checksum) -> Result<()> {
    if expected.algorithm != "blake3" {
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
    fn new(_original_size: u64) -> Self {
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
            .checked_add(u64::try_from(starts.len()).map_err(|_| QztError::ResourceLimitExceeded)?)
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
    let capacity = usize::try_from(expected_size).map_err(|_| QztError::ResourceLimitExceeded)?;
    let read_limit = expected_size
        .checked_add(1)
        .ok_or(QztError::ResourceLimitExceeded)?;
    let mut decoded = Vec::with_capacity(capacity);
    let mut limited = decoder.take(read_limit);
    limited
        .read_to_end(&mut decoded)
        .map_err(|_| QztError::ZstdDecodeError)?;

    if u64::try_from(decoded.len()).map_err(|_| QztError::ResourceLimitExceeded)? > expected_size {
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
    line_count: u64,
    chunk_entries: &[ChunkEntry],
    mut decode_entry: impl FnMut(&ChunkEntry) -> Result<Vec<u8>>,
) -> Result<()> {
    for document in &document_index.documents {
        let end = checked_logical_end(document.logical_offset, document.byte_length)?;
        let original_size = chunk_entries
            .last()
            .map(|entry| checked_logical_end(entry.logical_offset, entry.uncompressed_size))
            .transpose()?
            .unwrap_or(0);
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

        let bytes = read_range_from_entries(
            chunk_entries,
            original_size,
            document.logical_offset,
            document.byte_length,
            |entry| decode_entry(entry),
        )?;
        if Checksum::blake3(&bytes) != document.checksum {
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

fn document_chunk_range(
    chunk_entries: &[ChunkEntry],
    offset: u64,
    length: u64,
) -> Result<(u64, u64)> {
    let end = checked_logical_end(offset, length)?;
    if length == 0 {
        return Ok((0, 0));
    }

    let mut first = None;
    let mut last_exclusive = None;
    for entry in chunk_entries {
        let chunk_end = checked_logical_end(entry.logical_offset, entry.uncompressed_size)?;
        if chunk_end > offset && entry.logical_offset < end {
            first.get_or_insert(entry.chunk_id);
            last_exclusive = Some(
                entry
                    .chunk_id
                    .checked_add(1)
                    .ok_or(QztError::ChunkTableInvalid)?,
            );
        }
    }

    match (first, last_exclusive) {
        (Some(first), Some(last_exclusive)) => Ok((first, last_exclusive)),
        _ => Err(QztError::ChunkTableInvalid),
    }
}
