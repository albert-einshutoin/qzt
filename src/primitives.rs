use crate::error::{QztError, Result};

/// Converts a `usize` to `u64`. Returns `ResourceLimitExceeded` on overflow.
pub(crate) fn usize_to_u64(value: usize) -> Result<u64> {
    u64::try_from(value).map_err(|_| QztError::ResourceLimitExceeded)
}

/// Converts a `u64` offset or length to `usize`. Returns `ResourceLimitExceeded` on overflow.
pub(crate) fn u64_to_usize(value: u64) -> Result<usize> {
    usize::try_from(value).map_err(|_| QztError::ResourceLimitExceeded)
}

/// Reads a little-endian u16 from an exact two-byte slice.
pub fn read_u16_le(bytes: &[u8]) -> Result<u16> {
    let bytes: [u8; 2] = bytes.try_into().map_err(|_| QztError::UnexpectedEof)?;
    Ok(u16::from_le_bytes(bytes))
}

/// Reads a little-endian u32 from an exact four-byte slice.
pub fn read_u32_le(bytes: &[u8]) -> Result<u32> {
    let bytes: [u8; 4] = bytes.try_into().map_err(|_| QztError::UnexpectedEof)?;
    Ok(u32::from_le_bytes(bytes))
}

/// Reads a little-endian u64 from an exact eight-byte slice.
pub fn read_u64_le(bytes: &[u8]) -> Result<u64> {
    let bytes: [u8; 8] = bytes.try_into().map_err(|_| QztError::UnexpectedEof)?;
    Ok(u64::from_le_bytes(bytes))
}

/// Writes a little-endian u16.
#[must_use]
pub fn write_u16_le(value: u16) -> [u8; 2] {
    value.to_le_bytes()
}

/// Writes a little-endian u32.
#[must_use]
pub fn write_u32_le(value: u32) -> [u8; 4] {
    value.to_le_bytes()
}

/// Writes a little-endian u64.
#[must_use]
pub fn write_u64_le(value: u64) -> [u8; 8] {
    value.to_le_bytes()
}

/// Returns the exclusive end offset for a logical half-open range.
pub fn checked_logical_end(offset: u64, size: u64) -> Result<u64> {
    offset
        .checked_add(size)
        .ok_or(QztError::LogicalRangeOutOfBounds)
}

/// Returns the exclusive end offset for a physical half-open range.
pub fn checked_physical_end(offset: u64, size: u64) -> Result<u64> {
    offset
        .checked_add(size)
        .ok_or(QztError::PhysicalRangeOutOfBounds)
}
