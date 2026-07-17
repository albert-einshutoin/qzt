use std::time::Instant;

use qzt::dense_line_index::{DenseLineEntry, DenseLineIndex};
use qzt::error::QztError;
use qzt::reader::{QztReader, VerifyLevel};
use qzt::schema::{Checksum, DocumentEntry, DocumentIndex};
use qzt::skeleton::open_skeleton_details;
use qzt::writer::{
    pack_bytes_with_dense_line_index, pack_bytes_with_dense_line_index_override,
    pack_bytes_with_memory_profile,
};
mod support;

fn newline_corpus(line_count: u64) -> Vec<u8> {
    let mut input = Vec::new();
    for index in 0..line_count {
        input.extend_from_slice(format!("line-{index:04}\n").as_bytes());
    }
    input
}

fn single_document_memory_profile_index(
    input: &[u8],
    container_id: [u8; 16],
    line_count: u64,
) -> DocumentIndex {
    DocumentIndex {
        container_id,
        documents: vec![DocumentEntry::new(
            "all",
            0,
            input.len() as u64,
            0,
            line_count,
            0,
            1,
            Checksum::blake3(input),
        )],
    }
}

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
fn memory_profile_below_dense_threshold_omits_dense_line_index() {
    let line_count = 1024;
    let container_id = [0xb5; 16];
    let input = newline_corpus(line_count);
    let document_index = single_document_memory_profile_index(&input, container_id, line_count);
    let options = support::writer_options(65_536, 65_536);

    let container = pack_bytes_with_memory_profile(&input, container_id, options, &document_index)
        .expect("memory profile should pack");
    let details = open_skeleton_details(&container).expect("memory profile should open");

    assert_eq!(details.metadata.profile, "memory");
    assert!(!details.metadata.dense_line_index);
    assert!(details.dense_line_index.is_none());
    assert!(details.metadata.document_index);
    assert!(details.document_index.is_some());

    let reader = QztReader::open(container).expect("memory profile reader should open");
    assert_eq!(reader.read_line_raw(0), Ok(b"line-0000\n".to_vec()));
    assert_eq!(
        reader.read_line_raw(line_count - 1),
        Ok(format!("line-{:04}\n", line_count - 1).into_bytes())
    );
    assert!(reader.verify(VerifyLevel::Deep).is_ok());
}

#[test]
fn memory_profile_at_dense_threshold_writes_dense_line_index() {
    let line_count = 2048;
    let container_id = [0xb6; 16];
    let input = newline_corpus(line_count);
    let document_index = single_document_memory_profile_index(&input, container_id, line_count);
    let options = support::writer_options(65_536, 65_536);

    let container = pack_bytes_with_memory_profile(&input, container_id, options, &document_index)
        .expect("memory profile should pack");
    let details = open_skeleton_details(&container).expect("memory profile should open");

    assert_eq!(details.metadata.profile, "memory");
    assert!(details.metadata.dense_line_index);
    assert!(details.dense_line_index.is_some());
    assert!(details.metadata.document_index);
    assert!(details.document_index.is_some());

    let reader = QztReader::open(container).expect("memory profile reader should open");
    assert_eq!(reader.read_line_raw(0), Ok(b"line-0000\n".to_vec()));
    assert_eq!(
        reader.read_line_raw(line_count - 1),
        Ok(format!("line-{:04}\n", line_count - 1).into_bytes())
    );
    assert!(reader.verify(VerifyLevel::Deep).is_ok());
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
