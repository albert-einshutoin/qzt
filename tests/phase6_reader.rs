use qzt::chunker::ChunkerOptions;
use qzt::error::QztError;
use qzt::reader::{QztReader, VerifyLevel};
use qzt::skeleton::open_skeleton_details;
use qzt::writer::{pack_bytes_with_container_id, WriterOptions};

fn options() -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size: 8,
            max_chunk_size: 16,
        },
        zstd_level: 0,
    }
}

fn pack(input: &[u8]) -> Vec<u8> {
    pack_bytes_with_container_id(input, [0x66; 16], options()).expect("pack should work")
}

#[test]
fn reader_open_info_and_export_work_for_phase5_container() {
    let input = b"hello\nworld\n";
    let reader = QztReader::open(pack(input)).expect("reader should open");
    let info = reader.info();

    assert_eq!(info.original_size, input.len() as u64);
    assert_eq!(info.line_count, 2);
    assert_eq!(reader.export_all(), Ok(input.to_vec()));

    let mut exported = Vec::new();
    reader
        .export_to(&mut exported)
        .expect("export_to should work");
    assert_eq!(exported, input);
}

#[test]
fn quick_verify_succeeds_without_decompressing_corrupt_compressed_chunk() {
    let mut container = pack(b"hello\nworld");
    let details = open_skeleton_details(&container).expect("container should open structurally");
    let offset = details.chunk_entries[0].physical_offset as usize;
    container[offset] ^= 0xff;

    let reader = QztReader::open(container).expect("quick structural open should not decompress");
    let report = reader
        .verify(VerifyLevel::Quick)
        .expect("quick verify should pass");

    assert_eq!(report.level, VerifyLevel::Quick);
    assert_eq!(report.decoded_bytes, 0);
}

#[test]
fn normal_verify_detects_compressed_chunk_checksum_mismatch() {
    let mut container = pack(b"hello\nworld");
    let details = open_skeleton_details(&container).expect("container should open structurally");
    let offset = details.chunk_entries[0].physical_offset as usize;
    container[offset] ^= 0xff;

    let reader = QztReader::open(container).expect("open should remain structural");

    assert_eq!(
        reader.verify(VerifyLevel::Normal),
        Err(QztError::CompressedChunkChecksumMismatch)
    );
}

#[test]
fn normal_verify_detects_container_checksum_mismatch_when_present() {
    let mut container = pack(b"hello\nworld");
    container[40] ^= 0x01; // index_hint_offset is non-authoritative, but covered by container_checksum.

    let reader = QztReader::open(container).expect("index hint corruption should not break open");

    assert_eq!(
        reader.verify(VerifyLevel::Normal),
        Err(QztError::ContainerCorrupt)
    );
}

#[test]
fn deep_verify_decompresses_and_reports_decoded_bytes() {
    let input = b"a\r\nb\nc";
    let reader = QztReader::open(pack(input)).expect("reader should open");
    let report = reader
        .verify(VerifyLevel::Deep)
        .expect("deep verify should pass");

    assert_eq!(report.level, VerifyLevel::Deep);
    assert_eq!(report.decoded_bytes, input.len() as u64);
}
