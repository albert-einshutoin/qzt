use crate::chunk_table::{ChunkEntry, STARTS_WITH_LINE_CONTINUATION};
use crate::error::{QztError, Result};
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
        let mut cursor = 0_usize;
        let entry_count = read_varuint(bytes, &mut cursor)?;
        if entry_count != chunk_entries.len() as u64 {
            return Err(QztError::ChunkTableInvalid);
        }

        let mut entries = Vec::with_capacity(chunk_entries.len());
        for expected in chunk_entries {
            let chunk_id = read_varuint(bytes, &mut cursor)?;
            if chunk_id != expected.chunk_id {
                return Err(QztError::ChunkTableInvalid);
            }
            let offset_count = read_varuint(bytes, &mut cursor)?;
            if offset_count != expected.line_count {
                return Err(QztError::ChunkTableInvalid);
            }

            let mut offsets = Vec::with_capacity(u64_to_usize(offset_count)?);
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

fn write_varuint(mut value: u64, output: &mut Vec<u8>) {
    while value >= 0x80 {
        output.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

fn read_varuint(bytes: &[u8], cursor: &mut usize) -> Result<u64> {
    let start = *cursor;
    let mut value = 0_u64;
    let mut shift = 0_u32;

    loop {
        let byte = *bytes.get(*cursor).ok_or(QztError::UnexpectedEof)?;
        *cursor += 1;

        if shift >= 64 && byte & 0x7f != 0 {
            return Err(QztError::ChunkTableInvalid);
        }
        value |= u64::from(byte & 0x7f)
            .checked_shl(shift)
            .ok_or(QztError::ChunkTableInvalid)?;

        if byte & 0x80 == 0 {
            let mut minimal = Vec::new();
            write_varuint(value, &mut minimal);
            if minimal.as_slice() != &bytes[start..*cursor] {
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
