use proptest::prelude::*;

use qzt::error::QztError;
use qzt::primitives::{
    checked_logical_end, checked_physical_end, read_u16_le, read_u32_le, read_u64_le,
};

#[test]
fn little_endian_fixtures_decode() {
    assert_eq!(read_u16_le(&[0x34, 0x12]), Ok(0x1234));
    assert_eq!(read_u32_le(&[0x78, 0x56, 0x34, 0x12]), Ok(0x1234_5678));
    assert_eq!(
        read_u64_le(&[0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01]),
        Ok(0x0123_4567_89ab_cdef)
    );
}

#[test]
fn short_primitive_reads_are_fallible() {
    assert_eq!(read_u16_le(&[0]), Err(QztError::UnexpectedEof));
    assert_eq!(read_u32_le(&[0, 1, 2]), Err(QztError::UnexpectedEof));
    assert_eq!(
        read_u64_le(&[0, 1, 2, 3, 4, 5, 6]),
        Err(QztError::UnexpectedEof)
    );
}

#[test]
fn checked_range_overflow_uses_specific_errors() {
    assert_eq!(
        checked_logical_end(u64::MAX, 1),
        Err(QztError::LogicalRangeOutOfBounds)
    );
    assert_eq!(
        checked_physical_end(u64::MAX, 1),
        Err(QztError::PhysicalRangeOutOfBounds)
    );
}

proptest! {
    #[test]
    fn u16_round_trip(value in any::<u16>()) {
        prop_assert_eq!(read_u16_le(&value.to_le_bytes()), Ok(value));
    }

    #[test]
    fn u32_round_trip(value in any::<u32>()) {
        prop_assert_eq!(read_u32_le(&value.to_le_bytes()), Ok(value));
    }

    #[test]
    fn u64_round_trip(value in any::<u64>()) {
        prop_assert_eq!(read_u64_le(&value.to_le_bytes()), Ok(value));
    }

    #[test]
    fn checked_logical_end_matches_checked_add(offset in any::<u64>(), size in any::<u64>()) {
        let expected = offset.checked_add(size).ok_or(QztError::LogicalRangeOutOfBounds);
        prop_assert_eq!(checked_logical_end(offset, size), expected);
    }

    #[test]
    fn checked_physical_end_matches_checked_add(offset in any::<u64>(), size in any::<u64>()) {
        let expected = offset.checked_add(size).ok_or(QztError::PhysicalRangeOutOfBounds);
        prop_assert_eq!(checked_physical_end(offset, size), expected);
    }
}
