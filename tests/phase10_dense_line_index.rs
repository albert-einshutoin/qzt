use std::time::Instant;

use qzt::dense_line_index::{DenseLineEntry, DenseLineIndex};
use qzt::error::QztError;
use qzt::reader::{QztReader, VerifyLevel};
use qzt::skeleton::open_skeleton_details;
use qzt::writer::{pack_bytes_with_dense_line_index, pack_bytes_with_dense_line_index_override};
mod support;

#[test]
fn dense_line_index_reads_final_line_without_newline() {
    let input = b"alpha\nbeta\ngamma";
    let container =
        pack_bytes_with_dense_line_index(input, [0xa0; 16], support::writer_options(8, 8))
            .expect("dense container should pack");
    let reader = QztReader::open(container).expect("dense container should open");

    assert_eq!(reader.read_line_raw(2), Ok(b"gamma".to_vec()));
    assert_eq!(
        reader
            .verify(VerifyLevel::Deep)
            .expect("deep verify should pass")
            .decoded_bytes,
        input.len() as u64
    );
}

#[test]
fn dense_line_index_count_mismatch_is_rejected() {
    let input = b"alpha\nbeta\n";
    let mut container =
        pack_bytes_with_dense_line_index(input, [0xa1; 16], support::writer_options(32, 32))
            .expect("dense container should pack");
    let details = open_skeleton_details(&container).expect("container should open");
    let dense = details
        .dense_line_index
        .expect("dense line index should be present");
    let mut entries = dense.entries;
    entries[0].line_start_offsets.pop();
    container = pack_bytes_with_dense_line_index_override(
        input,
        [0xa1; 16],
        support::writer_options(32, 32),
        DenseLineIndex { entries },
    )
    .expect("corrupt dense container should pack structurally");

    assert_eq!(
        QztReader::open(container).map(|_| ()),
        Err(QztError::ChunkTableInvalid)
    );
}

#[test]
fn deep_verify_detects_dense_line_index_disagreement() {
    let input = b"alpha\nbeta\ngamma\n";
    let dense = DenseLineIndex {
        entries: vec![DenseLineEntry {
            chunk_id: 0,
            line_start_offsets: vec![0, 7, 12],
        }],
    };
    let container = pack_bytes_with_dense_line_index_override(
        input,
        [0xa2; 16],
        support::writer_options(64, 64),
        dense,
    )
    .expect("stale dense container should pack structurally");
    let reader = QztReader::open(container).expect("stale dense count should open");

    assert_eq!(
        reader.verify(VerifyLevel::Deep),
        Err(QztError::ChunkTableInvalid)
    );
}

#[test]
fn sparse_vs_dense_line_lookup_benchmark_records_threshold_evidence() {
    let line_count = 2048;
    let mut input = Vec::new();
    for index in 0..line_count {
        input.extend_from_slice(format!("line-{index:04}\n").as_bytes());
    }

    let sparse_container = qzt::writer::pack_bytes_with_container_id(
        &input,
        [0xa3; 16],
        support::writer_options(128, 128),
    )
    .expect("sparse container should pack");
    let dense_container =
        pack_bytes_with_dense_line_index(&input, [0xa4; 16], support::writer_options(128, 128))
            .expect("dense container should pack");
    let sparse_reader = QztReader::open(sparse_container).expect("sparse should open");
    let dense_reader = QztReader::open(dense_container).expect("dense should open");

    let started = Instant::now();
    for line in (0..line_count).step_by(7) {
        let _ = sparse_reader
            .read_line_raw(line)
            .expect("sparse line should read");
    }
    let sparse_elapsed = started.elapsed();

    let started = Instant::now();
    for line in (0..line_count).step_by(7) {
        let _ = dense_reader
            .read_line_raw(line)
            .expect("dense line should read");
    }
    let dense_elapsed = started.elapsed();

    assert!(sparse_elapsed.as_nanos() > 0);
    assert!(dense_elapsed.as_nanos() > 0);
    eprintln!(
        "phase10_dense_bench lines={} sparse_us={:.3} dense_us={:.3} threshold_decision=enable_dense_for_memory_profile_at_or_above_2048_lines",
        line_count,
        sparse_elapsed.as_secs_f64() * 1_000_000.0,
        dense_elapsed.as_secs_f64() * 1_000_000.0
    );
}
