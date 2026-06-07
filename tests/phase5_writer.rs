use qzt::chunk_table::STARTS_WITH_LINE_CONTINUATION;
use qzt::chunker::ChunkerOptions;
use qzt::error::QztError;
use qzt::fixed::Header;
use qzt::format::HEADER_LEN;
use qzt::skeleton::open_skeleton_details;
use qzt::writer::{export_all, pack_bytes_with_container_id, WriterOptions};
use std::time::Instant;

fn options(target_chunk_size: usize, max_chunk_size: usize) -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size,
            max_chunk_size,
        },
        zstd_level: 0,
    }
}

fn pack(input: &[u8]) -> Vec<u8> {
    pack_bytes_with_container_id(input, [0x55; 16], options(8, 16)).expect("pack should work")
}

#[test]
fn empty_file_pack_export_equality() {
    let container = pack(b"");

    assert_eq!(export_all(&container), Ok(Vec::new()));
    assert_eq!(
        open_skeleton_details(&container)
            .unwrap()
            .summary
            .chunk_count,
        0
    );
}

#[test]
fn ascii_file_pack_export_equality() {
    let input = b"hello\nworld\n";
    let container = pack(input);

    assert_eq!(export_all(&container), Ok(input.to_vec()));
}

#[test]
fn japanese_and_emoji_pack_export_equality() {
    let input = "こんにちは\n😀😃😄\n".as_bytes();
    let container = pack(input);

    assert_eq!(export_all(&container), Ok(input.to_vec()));
}

#[test]
fn crlf_and_mixed_newline_pack_export_equality() {
    let input = b"a\r\nb\nc";
    let container = pack(input);

    assert_eq!(export_all(&container), Ok(input.to_vec()));
}

#[test]
fn long_line_pack_export_equality_and_continuation_flags() {
    let input = b"abcdefghijklmnopqrstuvwxyz";
    let container =
        pack_bytes_with_container_id(input, [0x56; 16], options(8, 8)).expect("pack should work");
    let details = open_skeleton_details(&container).expect("container should open");

    assert_eq!(export_all(&container), Ok(input.to_vec()));
    assert!(details.summary.chunk_count > 1);
    assert_eq!(details.chunk_entries[0].flags, 0);
    assert_eq!(
        details.chunk_entries[1].flags,
        STARTS_WITH_LINE_CONTINUATION
    );
}

#[test]
fn compressed_and_uncompressed_checksums_match_exact_bytes() {
    let input = b"hello\nworld";
    let container = pack(input);
    let details = open_skeleton_details(&container).expect("container should open");

    for entry in details.chunk_entries {
        let start = entry.physical_offset as usize;
        let end = start + entry.compressed_size as usize;
        let compressed = &container[start..end];
        assert_eq!(
            *blake3::hash(compressed).as_bytes(),
            entry.compressed_checksum_blake3
        );

        let decoded = zstd::stream::decode_all(compressed).expect("chunk should decode");
        assert_eq!(
            *blake3::hash(&decoded).as_bytes(),
            entry.uncompressed_checksum_blake3
        );
    }
}

#[test]
fn header_is_patched_with_metadata_and_index_hint_offsets() {
    let container = pack(b"abc");
    let header = Header::decode(&container[..HEADER_LEN]).expect("header should decode");

    assert!(header.metadata_offset > HEADER_LEN as u64);
    assert!(header.metadata_size > 0);
    assert!(header.index_hint_offset > header.metadata_offset);
}

#[test]
fn writer_rejects_invalid_utf8() {
    assert_eq!(
        pack_bytes_with_container_id(&[0xff], [0x57; 16], options(8, 16)).map(|_| ()),
        Err(QztError::InvalidUtf8)
    );
}

#[test]
fn metadata_records_writer_options_used_for_pack() {
    let input = b"alpha\nbeta\n";
    let writer_options = WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size: 8,
            max_chunk_size: 8,
        },
        zstd_level: 3,
    };
    let container =
        pack_bytes_with_container_id(input, [0x59; 16], writer_options).expect("pack should work");
    let details = open_skeleton_details(&container).expect("container should open");

    assert_eq!(details.metadata.zstd_level, 3);
    assert_eq!(details.metadata.target_chunk_size, 8);
    assert_eq!(details.metadata.max_chunk_size, 8);
}

#[test]
fn pack_smoke_benchmark_records_nonzero_throughput() {
    let input = vec![b'a'; 64 * 1024];
    let started = Instant::now();
    let container = pack_bytes_with_container_id(&input, [0x58; 16], options(16 * 1024, 16 * 1024))
        .expect("pack should work");
    let elapsed = started.elapsed();

    assert!(!container.is_empty());
    assert!(elapsed.as_nanos() > 0);

    let bytes_per_second = input.len() as f64 / elapsed.as_secs_f64();
    eprintln!(
        "phase5_pack_smoke bytes={} elapsed_ms={:.3} throughput_mib_s={:.3}",
        input.len(),
        elapsed.as_secs_f64() * 1000.0,
        bytes_per_second / (1024.0 * 1024.0)
    );
}
