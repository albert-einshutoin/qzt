use qzt::error::QztError;
use qzt::fixed::{FooterTrailer, Header, PhysicalRange, validate_physical_ranges};
use qzt::format::{FOOTER_TRAILER_LEN, HEADER_LEN, MAGIC, TRAILER_MAGIC};

fn sample_header() -> Header {
    Header {
        metadata_offset: 256,
        metadata_size: 64,
        index_hint_offset: 512,
        container_id: [0x42; 16],
    }
}

fn sample_footer_trailer() -> FooterTrailer {
    FooterTrailer {
        footer_payload_offset: 1024,
        footer_payload_size: 128,
        footer_payload_checksum_blake3: [0x7a; 32],
    }
}

#[test]
fn header_round_trips_with_exact_layout() {
    let header = sample_header();
    let bytes = header.encode();

    assert_eq!(bytes.len(), HEADER_LEN);
    assert_eq!(&bytes[0..8], &MAGIC);
    assert_eq!(&bytes[8..10], &[0, 0]);
    assert_eq!(&bytes[10..12], &[1, 0]);
    assert_eq!(&bytes[12..16], &[128, 0, 0, 0]);
    assert_eq!(&bytes[16..24], &[0; 8]);
    assert_eq!(&bytes[24..32], &256_u64.to_le_bytes());
    assert_eq!(&bytes[32..40], &64_u64.to_le_bytes());
    assert_eq!(&bytes[40..48], &512_u64.to_le_bytes());
    assert_eq!(&bytes[48..64], &[0x42; 16]);
    assert_eq!(&bytes[64..128], &[0; 64]);

    assert_eq!(Header::decode(&bytes), Ok(header));
}

#[test]
fn header_rejects_invalid_magic_flags_reserved_and_version() {
    let mut bytes = sample_header().encode();
    bytes[0] = b'X';
    assert_eq!(Header::decode(&bytes), Err(QztError::InvalidMagic));

    let mut bytes = sample_header().encode();
    bytes[16] = 1;
    assert_eq!(Header::decode(&bytes), Err(QztError::InvalidFlags));

    let mut bytes = sample_header().encode();
    bytes[64] = 1;
    assert_eq!(Header::decode(&bytes), Err(QztError::InvalidHeader));

    let mut bytes = sample_header().encode();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(Header::decode(&bytes), Err(QztError::UnsupportedVersion));
}

#[test]
fn invalid_index_hint_offset_is_non_authoritative_for_header_decode() {
    let mut header = sample_header();
    header.index_hint_offset = u64::MAX;

    assert_eq!(Header::decode(&header.encode()), Ok(header));
}

#[test]
fn footer_trailer_round_trips_with_exact_layout() {
    let trailer = sample_footer_trailer();
    let bytes = trailer.encode();

    assert_eq!(bytes.len(), FOOTER_TRAILER_LEN);
    assert_eq!(&bytes[0..8], &TRAILER_MAGIC);
    assert_eq!(&bytes[8..10], &[0, 0]);
    assert_eq!(&bytes[10..12], &[1, 0]);
    assert_eq!(&bytes[12..16], &[64, 0, 0, 0]);
    assert_eq!(&bytes[16..24], &1024_u64.to_le_bytes());
    assert_eq!(&bytes[24..32], &128_u64.to_le_bytes());
    assert_eq!(&bytes[32..64], &[0x7a; 32]);

    assert_eq!(FooterTrailer::decode(&bytes), Ok(trailer));
}

#[test]
fn footer_trailer_rejects_corrupt_magic_length_and_version() {
    let mut bytes = sample_footer_trailer().encode();
    bytes[0] = b'X';
    assert_eq!(
        FooterTrailer::decode(&bytes),
        Err(QztError::InvalidFooterTrailer)
    );

    let mut bytes = sample_footer_trailer().encode();
    bytes[12..16].copy_from_slice(&63_u32.to_le_bytes());
    assert_eq!(
        FooterTrailer::decode(&bytes),
        Err(QztError::InvalidFooterTrailer)
    );

    let mut bytes = sample_footer_trailer().encode();
    bytes[10..12].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        FooterTrailer::decode(&bytes),
        Err(QztError::UnsupportedVersion)
    );
}

#[test]
fn physical_ranges_reject_too_small_files_and_out_of_bounds_ranges() {
    assert_eq!(
        validate_physical_ranges((HEADER_LEN + FOOTER_TRAILER_LEN - 1) as u64, &[]),
        Err(QztError::InvalidFooterTrailer)
    );

    assert_eq!(
        validate_physical_ranges(256, &[PhysicalRange::new(200, 100)]),
        Err(QztError::PhysicalRangeOutOfBounds)
    );

    assert_eq!(
        validate_physical_ranges(256, &[PhysicalRange::new(u64::MAX, 1)]),
        Err(QztError::PhysicalRangeOutOfBounds)
    );
}

#[test]
fn physical_ranges_are_half_open_and_must_not_overlap_reserved_or_each_other() {
    assert_eq!(
        validate_physical_ranges(
            1024,
            &[
                PhysicalRange::new(128, 128),
                PhysicalRange::new(256, 128),
                PhysicalRange::new(384, 128),
            ],
        ),
        Ok(())
    );

    assert_eq!(
        validate_physical_ranges(1024, &[PhysicalRange::new(64, 128)]),
        Err(QztError::RangeOverlap)
    );

    assert_eq!(
        validate_physical_ranges(1024, &[PhysicalRange::new(900, 80)]),
        Err(QztError::RangeOverlap)
    );

    assert_eq!(
        validate_physical_ranges(
            1024,
            &[PhysicalRange::new(128, 256), PhysicalRange::new(255, 1)],
        ),
        Err(QztError::RangeOverlap)
    );
}
