use crate::error::{QztError, Result};
use crate::format::{
    FOOTER_TRAILER_LEN, HEADER_LEN, MAGIC, MAJOR_VERSION, MINOR_VERSION, TRAILER_MAGIC,
};
use crate::primitives::{
    checked_physical_end, read_u16_le, read_u32_le, read_u64_le, write_u16_le, write_u32_le,
    write_u64_le,
};

/// Fixed QZT header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub metadata_offset: u64,
    pub metadata_size: u64,
    pub index_hint_offset: u64,
    pub container_id: [u8; 16],
}

impl Header {
    /// Decodes and validates a fixed 128-byte Header.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != HEADER_LEN {
            return Err(QztError::InvalidHeader);
        }

        if bytes[0..8] != MAGIC {
            return Err(QztError::InvalidMagic);
        }

        let major = read_u16_le(&bytes[8..10])?;
        let minor = read_u16_le(&bytes[10..12])?;
        if major != MAJOR_VERSION || minor != MINOR_VERSION {
            return Err(QztError::UnsupportedVersion);
        }

        let header_length = read_u32_le(&bytes[12..16])?;
        if header_length != HEADER_LEN as u32 {
            return Err(QztError::InvalidHeader);
        }

        let header_flags = read_u64_le(&bytes[16..24])?;
        if header_flags != 0 {
            return Err(QztError::InvalidFlags);
        }

        if bytes[64..128].iter().any(|byte| *byte != 0) {
            return Err(QztError::InvalidHeader);
        }

        Ok(Self {
            metadata_offset: read_u64_le(&bytes[24..32])?,
            metadata_size: read_u64_le(&bytes[32..40])?,
            index_hint_offset: read_u64_le(&bytes[40..48])?,
            container_id: bytes[48..64]
                .try_into()
                .map_err(|_| QztError::InvalidHeader)?,
        })
    }

    /// Encodes a fixed 128-byte Header.
    #[must_use]
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut bytes = [0_u8; HEADER_LEN];
        bytes[0..8].copy_from_slice(&MAGIC);
        bytes[8..10].copy_from_slice(&write_u16_le(MAJOR_VERSION));
        bytes[10..12].copy_from_slice(&write_u16_le(MINOR_VERSION));
        bytes[12..16].copy_from_slice(&write_u32_le(HEADER_LEN as u32));
        bytes[24..32].copy_from_slice(&write_u64_le(self.metadata_offset));
        bytes[32..40].copy_from_slice(&write_u64_le(self.metadata_size));
        bytes[40..48].copy_from_slice(&write_u64_le(self.index_hint_offset));
        bytes[48..64].copy_from_slice(&self.container_id);
        bytes
    }
}

/// Fixed QZT footer trailer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FooterTrailer {
    pub footer_payload_offset: u64,
    pub footer_payload_size: u64,
    pub footer_payload_checksum_blake3: [u8; 32],
}

impl FooterTrailer {
    /// Decodes and validates a fixed 64-byte Footer Trailer.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FOOTER_TRAILER_LEN {
            return Err(QztError::InvalidFooterTrailer);
        }

        if bytes[0..8] != TRAILER_MAGIC {
            return Err(QztError::InvalidFooterTrailer);
        }

        let major = read_u16_le(&bytes[8..10])?;
        let minor = read_u16_le(&bytes[10..12])?;
        if major != MAJOR_VERSION || minor != MINOR_VERSION {
            return Err(QztError::UnsupportedVersion);
        }

        let trailer_length = read_u32_le(&bytes[12..16])?;
        if trailer_length != FOOTER_TRAILER_LEN as u32 {
            return Err(QztError::InvalidFooterTrailer);
        }

        Ok(Self {
            footer_payload_offset: read_u64_le(&bytes[16..24])?,
            footer_payload_size: read_u64_le(&bytes[24..32])?,
            footer_payload_checksum_blake3: bytes[32..64]
                .try_into()
                .map_err(|_| QztError::InvalidFooterTrailer)?,
        })
    }

    /// Encodes a fixed 64-byte Footer Trailer.
    #[must_use]
    pub fn encode(&self) -> [u8; FOOTER_TRAILER_LEN] {
        let mut bytes = [0_u8; FOOTER_TRAILER_LEN];
        bytes[0..8].copy_from_slice(&TRAILER_MAGIC);
        bytes[8..10].copy_from_slice(&write_u16_le(MAJOR_VERSION));
        bytes[10..12].copy_from_slice(&write_u16_le(MINOR_VERSION));
        bytes[12..16].copy_from_slice(&write_u32_le(FOOTER_TRAILER_LEN as u32));
        bytes[16..24].copy_from_slice(&write_u64_le(self.footer_payload_offset));
        bytes[24..32].copy_from_slice(&write_u64_le(self.footer_payload_size));
        bytes[32..64].copy_from_slice(&self.footer_payload_checksum_blake3);
        bytes
    }
}

/// Half-open physical byte range `[offset, offset + size)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalRange {
    pub offset: u64,
    pub size: u64,
}

impl PhysicalRange {
    #[must_use]
    pub const fn new(offset: u64, size: u64) -> Self {
        Self { offset, size }
    }

    pub fn end(self) -> Result<u64> {
        checked_physical_end(self.offset, self.size)
    }

    pub fn is_inside_file(self, final_file_size: u64) -> Result<bool> {
        Ok(self.end()? <= final_file_size)
    }

    pub fn overlaps(self, other: Self) -> Result<bool> {
        let self_end = self.end()?;
        let other_end = other.end()?;
        Ok(self.offset < other_end && other.offset < self_end)
    }
}

/// Validates fixed Header/Footer capacity and non-overlapping physical ranges.
pub fn validate_physical_ranges(final_file_size: u64, ranges: &[PhysicalRange]) -> Result<()> {
    if final_file_size < (HEADER_LEN + FOOTER_TRAILER_LEN) as u64 {
        return Err(QztError::InvalidFooterTrailer);
    }

    let header = PhysicalRange::new(0, HEADER_LEN as u64);
    let footer_trailer = PhysicalRange::new(
        final_file_size - FOOTER_TRAILER_LEN as u64,
        FOOTER_TRAILER_LEN as u64,
    );

    for range in ranges {
        if !range.is_inside_file(final_file_size)? {
            return Err(QztError::PhysicalRangeOutOfBounds);
        }

        if range.overlaps(header)? || range.overlaps(footer_trailer)? {
            return Err(QztError::RangeOverlap);
        }
    }

    let mut sorted_ranges = ranges.to_vec();
    sorted_ranges.sort_by_key(|range| (range.offset, range.size));
    for pair in sorted_ranges.windows(2) {
        if pair[0].overlaps(pair[1])? {
            return Err(QztError::RangeOverlap);
        }
    }

    Ok(())
}
