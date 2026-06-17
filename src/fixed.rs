use crate::error::{QztError, Result};
use crate::format::{
    FOOTER_TRAILER_LEN, HEADER_LEN, MAGIC, MAJOR_VERSION, MINOR_VERSION, TRAILER_MAGIC,
};
use crate::primitives::{
    checked_physical_end, read_u16_le, read_u32_le, read_u64_le,
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
        decode_fixed_prologue(
            bytes,
            MAGIC,
            HEADER_LEN,
            QztError::InvalidHeader,
            QztError::InvalidMagic,
        )?;

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
        encode_fixed_prologue(
            &mut bytes,
            MAGIC,
            u32::try_from(HEADER_LEN).expect("header len fit u32"),
        );
        bytes[24..32].copy_from_slice(&self.metadata_offset.to_le_bytes());
        bytes[32..40].copy_from_slice(&self.metadata_size.to_le_bytes());
        bytes[40..48].copy_from_slice(&self.index_hint_offset.to_le_bytes());
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
        decode_fixed_prologue(
            bytes,
            TRAILER_MAGIC,
            FOOTER_TRAILER_LEN,
            QztError::InvalidFooterTrailer,
            QztError::InvalidFooterTrailer,
        )?;

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
        encode_fixed_prologue(
            &mut bytes,
            TRAILER_MAGIC,
            u32::try_from(FOOTER_TRAILER_LEN).expect("FOOTER_TRAILER_LEN fits in u32"),
        );
        bytes[16..24].copy_from_slice(&self.footer_payload_offset.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.footer_payload_size.to_le_bytes());
        bytes[32..64].copy_from_slice(&self.footer_payload_checksum_blake3);
        bytes
    }
}

fn decode_fixed_prologue(
    bytes: &[u8],
    expected_magic: [u8; 8],
    expected_length: usize,
    length_error: QztError,
    magic_error: QztError,
) -> Result<()> {
    if bytes[0..8] != expected_magic {
        return Err(magic_error);
    }

    let major = read_u16_le(&bytes[8..10])?;
    let minor = read_u16_le(&bytes[10..12])?;
    if major != MAJOR_VERSION || minor != MINOR_VERSION {
        return Err(QztError::UnsupportedVersion);
    }

    let header_length = read_u32_le(&bytes[12..16])?;
    let expected_len = u32::try_from(expected_length).map_err(|_| length_error)?;
    if header_length != expected_len {
        return Err(length_error);
    }
    Ok(())
}

fn encode_fixed_prologue(bytes: &mut [u8], magic: [u8; 8], len: u32) {
    bytes[0..8].copy_from_slice(&magic);
    bytes[8..10].copy_from_slice(&MAJOR_VERSION.to_le_bytes());
    bytes[10..12].copy_from_slice(&MINOR_VERSION.to_le_bytes());
    bytes[12..16].copy_from_slice(&len.to_le_bytes());
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
