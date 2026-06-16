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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usize_to_u64_zero_roundtrips() {
        assert_eq!(usize_to_u64(0).unwrap(), 0_u64);
    }

    #[test]
    fn u64_to_usize_zero_roundtrips() {
        assert_eq!(u64_to_usize(0).unwrap(), 0_usize);
    }

    #[test]
    fn roundtrip_u32_max_boundary() {
        let value = u64::from(u32::MAX);
        let as_usize = u64_to_usize(value).unwrap();
        assert_eq!(usize_to_u64(as_usize).unwrap(), value);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn usize_max_fits_in_u64_on_64_bit() {
        assert!(usize_to_u64(usize::MAX).is_ok());
    }
}
