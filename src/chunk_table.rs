use crate::error::{QztError, Result};
use crate::primitives::{read_u32_le, read_u64_le, usize_to_u64, write_u32_le, write_u64_le};

pub const CHUNK_ENTRY_LEN: usize = 128;
pub const STARTS_WITH_LINE_CONTINUATION: u32 = 1;

/// Fixed 128-byte Chunk Table entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkEntry {
    pub chunk_id: u64,
    pub physical_offset: u64,
    pub compressed_size: u64,
    pub logical_offset: u64,
    pub uncompressed_size: u64,
    pub first_line: u64,
    pub line_count: u64,
    pub dictionary_id: u32,
    pub flags: u32,
    pub compressed_checksum_blake3: [u8; 32],
    pub uncompressed_checksum_blake3: [u8; 32],
}

impl ChunkEntry {
    #[must_use]
    pub fn encode(&self) -> [u8; CHUNK_ENTRY_LEN] {
        let mut bytes = [0_u8; CHUNK_ENTRY_LEN];
        bytes[0..8].copy_from_slice(&write_u64_le(self.chunk_id));
        bytes[8..16].copy_from_slice(&write_u64_le(self.physical_offset));
        bytes[16..24].copy_from_slice(&write_u64_le(self.compressed_size));
        bytes[24..32].copy_from_slice(&write_u64_le(self.logical_offset));
        bytes[32..40].copy_from_slice(&write_u64_le(self.uncompressed_size));
        bytes[40..48].copy_from_slice(&write_u64_le(self.first_line));
        bytes[48..56].copy_from_slice(&write_u64_le(self.line_count));
        bytes[56..60].copy_from_slice(&write_u32_le(self.dictionary_id));
        bytes[60..64].copy_from_slice(&write_u32_le(self.flags));
        bytes[64..96].copy_from_slice(&self.compressed_checksum_blake3);
        bytes[96..128].copy_from_slice(&self.uncompressed_checksum_blake3);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CHUNK_ENTRY_LEN {
            return Err(QztError::ChunkTableInvalid);
        }

        Ok(Self {
            chunk_id: read_u64_le(&bytes[0..8])?,
            physical_offset: read_u64_le(&bytes[8..16])?,
            compressed_size: read_u64_le(&bytes[16..24])?,
            logical_offset: read_u64_le(&bytes[24..32])?,
            uncompressed_size: read_u64_le(&bytes[32..40])?,
            first_line: read_u64_le(&bytes[40..48])?,
            line_count: read_u64_le(&bytes[48..56])?,
            dictionary_id: read_u32_le(&bytes[56..60])?,
            flags: read_u32_le(&bytes[60..64])?,
            compressed_checksum_blake3: bytes[64..96]
                .try_into()
                .map_err(|_| QztError::ChunkTableInvalid)?,
            uncompressed_checksum_blake3: bytes[96..128]
                .try_into()
                .map_err(|_| QztError::ChunkTableInvalid)?,
        })
    }
}

/// Validates a fixed Chunk Table block without decompressing chunks.
pub fn validate_chunk_table_block(
    bytes: &[u8],
    chunk_count: u64,
    original_size: u64,
    line_count: u64,
) -> Result<Vec<ChunkEntry>> {
    if !bytes.len().is_multiple_of(CHUNK_ENTRY_LEN) {
        return Err(QztError::ChunkTableInvalid);
    }

    let actual_count = usize_to_u64(bytes.len() / CHUNK_ENTRY_LEN)?;
    if actual_count != chunk_count {
        return Err(QztError::ChunkCountMismatch);
    }

    if original_size == 0 {
        if chunk_count != 0 || line_count != 0 || !bytes.is_empty() {
            return Err(QztError::ChunkCountMismatch);
        }
        return Ok(Vec::new());
    }

    if chunk_count == 0 {
        return Err(QztError::ChunkCountMismatch);
    }

    let mut entries = Vec::with_capacity(bytes.len() / CHUNK_ENTRY_LEN);
    for record in bytes.chunks_exact(CHUNK_ENTRY_LEN) {
        entries.push(ChunkEntry::decode(record)?);
    }

    let mut expected_logical_offset = 0_u64;
    let mut expected_first_line = 0_u64;
    let mut total_uncompressed_size = 0_u64;
    let mut total_line_count = 0_u64;

    for (index, entry) in entries.iter().enumerate() {
        if entry.chunk_id != index as u64 {
            return Err(QztError::ChunkTableInvalid);
        }
        if entry.logical_offset != expected_logical_offset {
            return Err(QztError::ChunkTableInvalid);
        }
        if entry.first_line != expected_first_line {
            return Err(QztError::ChunkTableInvalid);
        }
        if entry.compressed_size == 0 || entry.uncompressed_size == 0 {
            return Err(QztError::ChunkSizeMismatch);
        }
        if entry.flags & !STARTS_WITH_LINE_CONTINUATION != 0 {
            return Err(QztError::InvalidFlags);
        }

        expected_logical_offset = expected_logical_offset
            .checked_add(entry.uncompressed_size)
            .ok_or(QztError::LogicalRangeOutOfBounds)?;
        expected_first_line = expected_first_line
            .checked_add(entry.line_count)
            .ok_or(QztError::LogicalRangeOutOfBounds)?;
        total_uncompressed_size = total_uncompressed_size
            .checked_add(entry.uncompressed_size)
            .ok_or(QztError::LogicalRangeOutOfBounds)?;
        total_line_count = total_line_count
            .checked_add(entry.line_count)
            .ok_or(QztError::LogicalRangeOutOfBounds)?;
    }

    if total_uncompressed_size != original_size || total_line_count != line_count {
        return Err(QztError::ChunkSizeMismatch);
    }

    Ok(entries)
}
