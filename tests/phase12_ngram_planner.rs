use std::fs;
use std::process::Command;

use qzt::chunker::ChunkerOptions;
use qzt::error::QztError;
use qzt::reader::QztReader;
use qzt::search::{
    NgramIndexBuildOptions, NgramUnit, RawNgramIndex, SearchIndexSource, SearchOptions,
};
use qzt::writer::{pack_bytes_with_container_id, WriterOptions};

fn options(target_chunk_size: usize, max_chunk_size: usize) -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size,
            max_chunk_size,
        },
        zstd_level: 0,
    }
}

#[test]
fn ngram_declaration_uses_raw_unicode_scalar_without_normalization() {
    let container =
        pack_bytes_with_container_id("東京大学\n".as_bytes(), [0xd0; 16], options(64, 64))
            .expect("container should pack");
    let index = RawNgramIndex::build_from_container(
        &container,
        NgramIndexBuildOptions {
            n: 2,
            ..NgramIndexBuildOptions::default()
        },
    )
    .expect("ngram index should build");

    assert_eq!(index.declaration.n, 2);
    assert_eq!(index.declaration.unit, NgramUnit::UnicodeScalar);
    assert_eq!(index.declaration.normalization, "none");
    assert!(!index.declaration.case_fold);
    assert_eq!(index.source, SearchIndexSource::RawUtf8);
}

#[test]
fn normalized_ngram_index_is_rejected_without_mapping_metadata() {
    let container = pack_bytes_with_container_id(b"alpha\n", [0xd1; 16], options(64, 64))
        .expect("container should pack");
    let error = RawNgramIndex::build_from_container(
        &container,
        NgramIndexBuildOptions {
            source: SearchIndexSource::NormalizedUtf8,
            ..NgramIndexBuildOptions::default()
        },
    )
    .expect_err("normalized ngram index is out of scope");

    assert_eq!(
        error,
        QztError::NotImplemented("normalized_utf8 ngram index")
    );
}

#[test]
fn ngram_search_verifies_substring_across_chunk_boundaries() {
    let input = "abc東京xyz\n京都abc\n";
    let container = pack_bytes_with_container_id(input.as_bytes(), [0xd2; 16], options(5, 5))
        .expect("container should pack");
    let index = RawNgramIndex::build_from_container(
        &container,
        NgramIndexBuildOptions {
            n: 2,
            ..NgramIndexBuildOptions::default()
        },
    )
    .expect("ngram index should build");
    let reader = QztReader::open(container).expect("reader should open");

    let report = index
        .search(&reader, "東京x", SearchOptions::default())
        .expect("search should run");

    assert_eq!(report.metrics.index_kind, "ngram");
    assert_eq!(report.metrics.verified_matches, 1);
    assert_eq!(report.hits[0].logical_offset, 3);
    assert_eq!(report.hits[0].byte_length, "東京x".len() as u64);
    assert_eq!(report.hits[0].source, "verified_original_bytes");
    assert!(report.metrics.candidate_chunks > 1);
}

#[test]
fn missing_key_in_complete_index_returns_no_match_without_decode() {
    let container = pack_bytes_with_container_id(b"alpha\nbeta\n", [0xd3; 16], options(64, 64))
        .expect("container should pack");
    let index = RawNgramIndex::build_from_container(
        &container,
        NgramIndexBuildOptions {
            n: 3,
            complete: true,
            ..NgramIndexBuildOptions::default()
        },
    )
    .expect("ngram index should build");
    let reader = QztReader::open(container).expect("reader should open");

    let report = index
        .search(&reader, "zzz", SearchOptions::default())
        .expect("search should run");

    assert!(report.hits.is_empty());
    assert_eq!(report.metrics.decoded_bytes, 0);
    assert!(!report.planner.missing_keys.is_empty());
    assert_eq!(report.incomplete_reason, None);
}

#[test]
fn missing_key_in_incomplete_index_reports_incomplete_without_fallback_decode() {
    let container = pack_bytes_with_container_id(b"alpha\nbeta\n", [0xd4; 16], options(64, 64))
        .expect("container should pack");
    let index = RawNgramIndex::build_from_container(
        &container,
        NgramIndexBuildOptions {
            n: 3,
            complete: false,
            ..NgramIndexBuildOptions::default()
        },
    )
    .expect("ngram index should build");
    let reader = QztReader::open(container).expect("reader should open");

    let report = index
        .search(&reader, "zzz", SearchOptions::default())
        .expect("search should run");

    assert!(report.hits.is_empty());
    assert_eq!(report.metrics.decoded_bytes, 0);
    assert_eq!(
        report.incomplete_reason,
        Some("missing_required_key_in_incomplete_index")
    );
}

#[test]
fn planner_uses_rarest_non_high_df_key_first() {
    let mut input = String::new();
    for _ in 0..64 {
        input.push_str("aaaxxx\n");
    }
    input.push_str("aaazzz\n");
    let container = pack_bytes_with_container_id(input.as_bytes(), [0xd5; 16], options(128, 128))
        .expect("container should pack");
    let index = RawNgramIndex::build_from_container(
        &container,
        NgramIndexBuildOptions {
            n: 3,
            high_df_per_million: 200_000,
            ..NgramIndexBuildOptions::default()
        },
    )
    .expect("ngram index should build");
    let reader = QztReader::open(container).expect("reader should open");

    let report = index
        .search(&reader, "aaazzz", SearchOptions::default())
        .expect("search should run");

    assert!(report.planner.high_df_keys.contains(&b"aaa".to_vec()));
    assert_ne!(report.planner.selected_keys.first(), Some(&b"aaa".to_vec()));
    assert_eq!(report.metrics.verified_matches, 1);
}

#[test]
fn skip_data_reduces_reported_posting_bytes_for_long_lists() {
    let mut input = String::new();
    for index in 0..1100 {
        input.push_str(&format!("aaa line {index}\n"));
    }
    let container = pack_bytes_with_container_id(input.as_bytes(), [0xd6; 16], options(512, 512))
        .expect("container should pack");
    let index = RawNgramIndex::build_from_container(
        &container,
        NgramIndexBuildOptions {
            n: 3,
            ..NgramIndexBuildOptions::default()
        },
    )
    .expect("ngram index should build");
    let term = index.term_for_key(b"aaa").expect("aaa term should exist");
    let reader = QztReader::open(container).expect("reader should open");

    let report = index
        .search(&reader, "aaa", SearchOptions::default())
        .expect("search should run");

    assert!(report.planner.used_skip_data);
    assert!(report.metrics.posting_bytes_read < term.posting_size);
}

#[test]
fn cli_ngram_search_reports_benchmark_metrics() {
    let base = std::env::temp_dir().join(format!("qzt-phase12-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    fs::write(&input, "東京大学\n京都大学\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );

    let output = output_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("search")
            .arg(&packed)
            .arg("東京")
            .arg("--index")
            .arg("ngram")
            .arg("--ngram")
            .arg("2"),
    );
    let output = String::from_utf8(output).expect("search output should be utf-8");

    assert!(output.contains("index_kind=ngram"));
    assert!(output.contains("candidate_granules="));
    assert!(output.contains("candidate_chunks="));
    assert!(output.contains("decoded_bytes="));
    assert!(output.contains("query_time_ms="));
    assert!(output.contains("index_size_ratio="));

    let _ = fs::remove_dir_all(base);
}

fn output_success(command: &mut Command) -> Vec<u8> {
    let output = command.output().expect("command should run");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn assert_success(command: &mut Command) {
    let output = command.output().expect("command should run");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
