use qzt::cbor::{CborValue, encode_deterministic};
use qzt::chunk_table::{CHUNK_ENTRY_LEN, ChunkEntry, validate_chunk_table_block};
use qzt::error::QztError;
use qzt::schema::{Checksum, FooterPayload, IndexRoot, Metadata, validate_source_consistency};
use qzt::skeleton::{open_skeleton, write_empty_container};

fn container_id() -> [u8; 16] {
    [0x11; 16]
}

#[test]
fn empty_container_writes_and_opens_structurally() {
    let bytes = write_empty_container(container_id()).expect("empty skeleton should write");
    let summary = open_skeleton(&bytes).expect("empty skeleton should open");

    assert_eq!(summary.container_id, container_id());
    assert_eq!(summary.original_size, 0);
    assert_eq!(summary.chunk_count, 0);
    assert_eq!(summary.line_count, 0);
}

#[test]
fn footer_payload_unknown_field_is_rejected() {
    let value = CborValue::Map(vec![
        (
            CborValue::Text("schema".to_owned()),
            CborValue::Text("qzt.footer.v1".to_owned()),
        ),
        (
            CborValue::Text("format_version".to_owned()),
            CborValue::Array(vec![CborValue::Integer(0), CborValue::Integer(1)]),
        ),
        (
            CborValue::Text("container_id".to_owned()),
            CborValue::Bytes(container_id().to_vec()),
        ),
        (CborValue::Text("unknown".to_owned()), CborValue::Null),
    ]);
    let bytes = encode_deterministic(&value).expect("test CBOR should encode");

    assert_eq!(
        FooterPayload::decode(&bytes),
        Err(QztError::InvalidFooterPayload)
    );
}

#[test]
fn footer_payload_checksum_mismatch_is_rejected() {
    let mut bytes = write_empty_container(container_id()).expect("empty skeleton should write");
    let checksum_byte = bytes.len() - 1;
    bytes[checksum_byte] ^= 0xff;

    assert_eq!(open_skeleton(&bytes), Err(QztError::FooterChecksumMismatch));
}

#[test]
fn header_footer_container_id_mismatch_is_rejected() {
    let mut bytes = write_empty_container(container_id()).expect("empty skeleton should write");
    bytes[48] ^= 0xff;

    assert_eq!(open_skeleton(&bytes), Err(QztError::ContainerIdMismatch));
}

#[test]
fn metadata_and_index_root_source_mismatch_is_rejected() {
    let metadata = Metadata::empty(container_id());
    let index_root = IndexRoot {
        container_id: container_id(),
        blocks: Vec::new(),
        original_size: 1,
        original_checksum: Checksum::blake3(b"x"),
        chunk_count: 0,
        line_count: 0,
    };

    assert_eq!(
        validate_source_consistency(&metadata, &index_root),
        Err(QztError::MetadataInvalid)
    );
}

#[test]
fn chunk_table_block_size_must_match_fixed_record_size() {
    assert_eq!(
        validate_chunk_table_block(&[0; CHUNK_ENTRY_LEN - 1], 1, 1, 1),
        Err(QztError::ChunkTableInvalid)
    );
}

#[test]
fn chunk_count_mismatch_is_rejected() {
    assert_eq!(
        validate_chunk_table_block(&[], 1, 1, 1),
        Err(QztError::ChunkCountMismatch)
    );
}

#[test]
fn empty_chunk_table_is_valid_for_empty_source() {
    assert_eq!(validate_chunk_table_block(&[], 0, 0, 0), Ok(Vec::new()));
}

#[test]
fn chunk_table_rejects_more_lines_than_source_bytes() {
    let entry = ChunkEntry {
        chunk_id: 0,
        physical_offset: 0,
        compressed_size: 1,
        logical_offset: 0,
        uncompressed_size: 1,
        first_line: 0,
        line_count: 1_u64 << 61,
        dictionary_id: 0,
        flags: 0,
        compressed_checksum_blake3: [0; 32],
        uncompressed_checksum_blake3: [0; 32],
    };
    assert_eq!(
        validate_chunk_table_block(&entry.encode(), 1, 1, entry.line_count),
        Err(QztError::ChunkTableInvalid)
    );
}

#[test]
fn chunk_table_rejects_cumulative_range_overflow() {
    let first = ChunkEntry {
        chunk_id: 0,
        physical_offset: 0,
        compressed_size: 1,
        logical_offset: 0,
        uncompressed_size: u64::MAX - 1,
        first_line: 0,
        line_count: u64::MAX - 1,
        dictionary_id: 0,
        flags: 0,
        compressed_checksum_blake3: [0; 32],
        uncompressed_checksum_blake3: [0; 32],
    };
    let mut second = first.clone();
    second.chunk_id = 1;
    second.logical_offset = u64::MAX - 1;
    second.first_line = u64::MAX - 1;
    second.uncompressed_size = 2;
    second.line_count = 2;
    let mut table = first.encode().to_vec();
    table.extend_from_slice(&second.encode());
    assert_eq!(
        validate_chunk_table_block(&table, 2, u64::MAX, u64::MAX),
        Err(QztError::LogicalRangeOutOfBounds)
    );
}

#[test]
fn chunk_table_rejects_per_chunk_line_excess_even_when_total_fits() {
    let first = ChunkEntry {
        chunk_id: 0,
        physical_offset: 0,
        compressed_size: 1,
        logical_offset: 0,
        uncompressed_size: 1,
        first_line: 0,
        line_count: 2,
        dictionary_id: 0,
        flags: 0,
        compressed_checksum_blake3: [0; 32],
        uncompressed_checksum_blake3: [0; 32],
    };
    let mut second = first.clone();
    second.chunk_id = 1;
    second.logical_offset = 1;
    second.uncompressed_size = 2;
    second.first_line = 2;
    second.line_count = 0;
    let mut table = first.encode().to_vec();
    table.extend_from_slice(&second.encode());
    assert_eq!(
        validate_chunk_table_block(&table, 2, 3, 2),
        Err(QztError::ChunkTableInvalid)
    );
}
