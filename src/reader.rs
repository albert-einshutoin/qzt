use std::io::{Read, Write};

use crate::chunk_table::{ChunkEntry, STARTS_WITH_LINE_CONTINUATION};
use crate::error::{QztError, Result};
use crate::fixed::PhysicalRange;
use crate::limits::ResourceLimits;
use crate::primitives::{checked_logical_end, checked_physical_end};
use crate::schema::Checksum;
use crate::skeleton::{open_skeleton_details, open_skeleton_details_with_limits, SkeletonDetails};

/// Reader for an in-memory QZT container.
pub struct QztReader {
    bytes: Vec<u8>,
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

        let mut output = Vec::new();
        for (chunk_index, entry) in self.details.chunk_entries.iter().enumerate() {
            let expected_flags = if entry.logical_offset > 0 && output.last() != Some(&b'\n') {
                STARTS_WITH_LINE_CONTINUATION
            } else {
                0
            };
            if entry.flags != expected_flags {
                return Err(QztError::ChunkTableInvalid);
            }

            let decoded = self.decode_entry(entry)?;
            if let Some(dense) = &self.details.dense_line_index {
                dense.verify_chunk(chunk_index, &decoded, entry.flags)?;
            }
            output.extend_from_slice(&decoded);
        }

        if u64::try_from(output.len()).map_err(|_| QztError::ResourceLimitExceeded)?
            != self.details.summary.original_size
        {
            return Err(QztError::ChunkSizeMismatch);
        }
        if Checksum::blake3(&output) != self.details.metadata.original_checksum {
            return Err(QztError::UncompressedChunkChecksumMismatch);
        }
        std::str::from_utf8(&output).map_err(|_| QztError::InvalidUtf8)?;

        let text = TextAnalysis::analyze(&output);
        if text.line_count != self.details.metadata.line_count {
            return Err(QztError::ContainerCorrupt);
        }
        if text.newline_mode != self.details.metadata.newline_mode {
            return Err(QztError::NewlineModeMismatch);
        }

        for entry in &self.details.chunk_entries {
            let start =
                usize::try_from(entry.logical_offset).map_err(|_| QztError::ContainerCorrupt)?;
            let end = start
                .checked_add(
                    usize::try_from(entry.uncompressed_size)
                        .map_err(|_| QztError::ContainerCorrupt)?,
                )
                .ok_or(QztError::ContainerCorrupt)?;
            let first_line = lower_bound(&text.line_starts, start);
            let line_end = lower_bound(&text.line_starts, end);
            if entry.first_line != first_line as u64
                || entry.line_count != u64::try_from(line_end - first_line).unwrap_or(u64::MAX)
            {
                return Err(QztError::ChunkTableInvalid);
            }
        }

        if let Some(document_index) = &self.details.document_index {
            verify_document_index(
                document_index,
                &output,
                text.line_count,
                &self.details.chunk_entries,
            )?;
        }

        Ok(VerifyReport {
            level: VerifyLevel::Deep,
            checked_chunks: self.details.summary.chunk_count,
            decoded_bytes: u64::try_from(output.len())
                .map_err(|_| QztError::ResourceLimitExceeded)?,
        })
    }

    fn decode_entry(&self, entry: &ChunkEntry) -> Result<Vec<u8>> {
        let compressed = self.slice_physical(PhysicalRange::new(
            entry.physical_offset,
            entry.compressed_size,
        ))?;
        if Checksum::blake3(compressed).value != entry.compressed_checksum_blake3 {
            return Err(QztError::CompressedChunkChecksumMismatch);
        }

        let dictionary = if entry.dictionary_id == 0 {
            &[][..]
        } else {
            self.details
                .dictionaries
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

struct TextAnalysis {
    line_starts: Vec<usize>,
    line_count: u64,
    newline_mode: String,
}

impl TextAnalysis {
    fn analyze(input: &[u8]) -> Self {
        if input.is_empty() {
            return Self {
                line_starts: Vec::new(),
                line_count: 0,
                newline_mode: "none".to_owned(),
            };
        }

        let mut line_starts = vec![0];
        let mut lf_count = 0_u64;
        let mut crlf_count = 0_u64;

        for index in 0..input.len() {
            if input[index] != b'\n' {
                continue;
            }

            if index > 0 && input[index - 1] == b'\r' {
                crlf_count += 1;
            } else {
                lf_count += 1;
            }

            if index + 1 < input.len() {
                line_starts.push(index + 1);
            }
        }

        let newline_mode = match (lf_count > 0, crlf_count > 0) {
            (false, false) => "none",
            (true, false) => "lf",
            (false, true) => "crlf",
            (true, true) => "mixed",
        }
        .to_owned();

        Self {
            line_count: line_starts.len() as u64,
            line_starts,
            newline_mode,
        }
    }
}

fn lower_bound(values: &[usize], target: usize) -> usize {
    values.partition_point(|value| *value < target)
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

fn verify_document_index(
    document_index: &crate::schema::DocumentIndex,
    original: &[u8],
    line_count: u64,
    chunk_entries: &[ChunkEntry],
) -> Result<()> {
    for document in &document_index.documents {
        let end = checked_logical_end(document.logical_offset, document.byte_length)?;
        if end > original.len() as u64 {
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

        let start =
            usize::try_from(document.logical_offset).map_err(|_| QztError::ContainerCorrupt)?;
        let end = usize::try_from(end).map_err(|_| QztError::ContainerCorrupt)?;
        let bytes = original.get(start..end).ok_or(QztError::ContainerCorrupt)?;
        if Checksum::blake3(bytes) != document.checksum {
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
