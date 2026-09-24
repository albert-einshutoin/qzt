use crate::chunk_table::{ChunkEntry, STARTS_WITH_LINE_CONTINUATION};
use crate::error::{QztError, Result};
use crate::limits::ResourceLimits;
use crate::primitives::{u64_to_usize, usize_to_u64};

/// Dense Line Index for fast in-chunk line start lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseLineIndex {
    pub entries: Vec<DenseLineEntry>,
}

/// Dense offsets for one chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseLineEntry {
    pub chunk_id: u64,
    pub line_start_offsets: Vec<u64>,
}

impl DenseLineIndex {
    pub fn from_original_bytes(input: &[u8], chunk_entries: &[ChunkEntry]) -> Result<Self> {
        let mut entries = Vec::with_capacity(chunk_entries.len());
        for entry in chunk_entries {
            let start =
                usize::try_from(entry.logical_offset).map_err(|_| QztError::ContainerCorrupt)?;
            let end = start
                .checked_add(
                    usize::try_from(entry.uncompressed_size)
                        .map_err(|_| QztError::ContainerCorrupt)?,
                )
                .ok_or(QztError::ContainerCorrupt)?;
            let decoded = input.get(start..end).ok_or(QztError::ContainerCorrupt)?;
            let line_start_offsets = line_start_offsets(decoded, entry.flags)?;
            if line_start_offsets.len() as u64 != entry.line_count {
                return Err(QztError::ChunkTableInvalid);
            }
            entries.push(DenseLineEntry {
                chunk_id: entry.chunk_id,
                line_start_offsets,
            });
        }
        Ok(Self { entries })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        write_varuint(usize_to_u64(self.entries.len())?, &mut bytes);
        for entry in &self.entries {
            write_varuint(entry.chunk_id, &mut bytes);
            write_varuint(usize_to_u64(entry.line_start_offsets.len())?, &mut bytes);
            let mut previous = 0_u64;
            for (index, offset) in entry.line_start_offsets.iter().enumerate() {
                let delta = if index == 0 {
                    *offset
                } else {
                    offset
                        .checked_sub(previous)
                        .ok_or(QztError::ChunkTableInvalid)?
                };
                write_varuint(delta, &mut bytes);
                previous = *offset;
            }
        }
        Ok(bytes)
    }

    pub fn decode_for_chunks(bytes: &[u8], chunk_entries: &[ChunkEntry]) -> Result<Self> {
        Self::decode_for_chunks_with_limit(
            bytes,
            chunk_entries,
            ResourceLimits::default().max_dense_line_index_allocation,
        )
    }

    pub fn decode_for_chunks_with_limit(
        bytes: &[u8],
        chunk_entries: &[ChunkEntry],
        max_allocation: u64,
    ) -> Result<Self> {
        let mut cursor = 0_usize;
        let entry_count = read_varuint(bytes, &mut cursor)?;
        if entry_count != usize_to_u64(chunk_entries.len())? {
            return Err(QztError::ChunkTableInvalid);
        }
        // Every entry needs at least a chunk ID and an offset count.
        if entry_count > usize_to_u64((bytes.len() - cursor) / 2)? {
            return Err(QztError::UnexpectedEof);
        }

        let mut requested = allocation_bytes::<DenseLineEntry>(chunk_entries.len())?;
        if requested > max_allocation {
            return Err(QztError::ResourceLimitExceeded);
        }

        let mut entries = Vec::new();
        reserve_exact(&mut entries, chunk_entries.len())?;
        for expected in chunk_entries {
            let chunk_id = read_varuint(bytes, &mut cursor)?;
            if chunk_id != expected.chunk_id {
                return Err(QztError::ChunkTableInvalid);
            }
            let offset_count = read_varuint(bytes, &mut cursor)?;
            if offset_count != expected.line_count {
                return Err(QztError::ChunkTableInvalid);
            }
            if offset_count > expected.uncompressed_size {
                return Err(QztError::ChunkTableInvalid);
            }
            // Each offset consumes at least one encoded byte.
            if offset_count > usize_to_u64(bytes.len() - cursor)? {
                return Err(QztError::UnexpectedEof);
            }

            let offset_count = u64_to_usize(offset_count)?;
            requested = requested
                .checked_add(allocation_bytes::<u64>(offset_count)?)
                .ok_or(QztError::ResourceLimitExceeded)?;
            if requested > max_allocation {
                return Err(QztError::ResourceLimitExceeded);
            }

            let mut offsets = Vec::new();
            reserve_exact(&mut offsets, offset_count)?;
            let mut previous = 0_u64;
            for index in 0..offset_count {
                let delta = read_varuint(bytes, &mut cursor)?;
                let offset = if index == 0 {
                    delta
                } else {
                    previous
                        .checked_add(delta)
                        .ok_or(QztError::ChunkTableInvalid)?
                };
                if index > 0 && offset <= previous {
                    return Err(QztError::ChunkTableInvalid);
                }
                if offset >= expected.uncompressed_size {
                    return Err(QztError::ChunkTableInvalid);
                }
                offsets.push(offset);
                previous = offset;
            }

            entries.push(DenseLineEntry {
                chunk_id,
                line_start_offsets: offsets,
            });
        }

        if cursor != bytes.len() {
            return Err(QztError::ChunkTableInvalid);
        }

        Ok(Self { entries })
    }

    pub fn line_start_offset(&self, chunk_index: usize, local_line_index: usize) -> Result<u64> {
        self.entries
            .get(chunk_index)
            .and_then(|entry| entry.line_start_offsets.get(local_line_index))
            .copied()
            .ok_or(QztError::LineOutOfRange)
    }

    pub fn verify_chunk(&self, chunk_index: usize, decoded: &[u8], flags: u32) -> Result<()> {
        let expected = line_start_offsets(decoded, flags)?;
        let actual = self
            .entries
            .get(chunk_index)
            .ok_or(QztError::ChunkTableInvalid)?;
        if actual.line_start_offsets != expected {
            return Err(QztError::ChunkTableInvalid);
        }
        Ok(())
    }
}

fn allocation_bytes<T>(count: usize) -> Result<u64> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(QztError::ResourceLimitExceeded)?;
    usize_to_u64(bytes)
}

fn reserve_exact<T>(values: &mut Vec<T>, count: usize) -> Result<()> {
    values
        .try_reserve_exact(count)
        .map_err(|_| QztError::ResourceLimitExceeded)
}

pub fn line_start_offsets(decoded: &[u8], flags: u32) -> Result<Vec<u64>> {
    let mut starts = Vec::new();
    if flags & STARTS_WITH_LINE_CONTINUATION == 0 && !decoded.is_empty() {
        starts.push(0);
    }

    for index in 0..decoded.len() {
        if decoded[index] == b'\n' && index + 1 < decoded.len() {
            starts.push(usize_to_u64(index + 1)?);
        }
    }

    Ok(starts)
}

#[allow(clippy::cast_possible_truncation)] // value ranges guaranteed by the loop invariants
fn write_varuint(mut value: u64, output: &mut Vec<u8>) {
    while value >= 0x80 {
        output.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

fn read_varuint(bytes: &[u8], cursor: &mut usize) -> Result<u64> {
    let mut value = 0_u64;
    let mut shift = 0_u32;
    let mut length = 0_u8;

    loop {
        let byte = *bytes.get(*cursor).ok_or(QztError::UnexpectedEof)?;
        *cursor += 1;
        length += 1;

        if shift == 63 && byte & 0x7f > 1 {
            return Err(QztError::ChunkTableInvalid);
        }
        value |= u64::from(byte & 0x7f)
            .checked_shl(shift)
            .ok_or(QztError::ChunkTableInvalid)?;

        if byte & 0x80 == 0 {
            if length > 1 && byte == 0 {
                return Err(QztError::ChunkTableInvalid);
            }
            return Ok(value);
        }

        shift += 7;
        if shift > 63 {
            return Err(QztError::ChunkTableInvalid);
        }
    }
}

#[cfg(test)]
mod allocation_tests {
    use super::*;

    #[test]
    fn reservation_failure_is_a_regular_error() {
        assert_eq!(
            reserve_exact(&mut Vec::<DenseLineEntry>::new(), usize::MAX),
            Err(QztError::ResourceLimitExceeded)
        );
        assert_eq!(
            reserve_exact(&mut Vec::<u64>::new(), usize::MAX),
            Err(QztError::ResourceLimitExceeded)
        );
    }
}
