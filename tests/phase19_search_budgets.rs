use std::fs;
use std::process::Command;

use qzt::{
    NgramIndexBuildOptions, QziFileSidecar, QztError, QztFileReader, QztReader, RawNgramIndex,
    RawTokenIndex, SearchOptions, SidecarIndexKind, TokenIndexBuildOptions, build_search_sidecar,
    build_search_sidecar_from_file_with_line_limit, pack_bytes,
};
mod support;
use support::{CountingReadAt, writer_options};

fn fixture(input: &[u8], chunk: usize) -> Vec<u8> {
    pack_bytes(input, writer_options(chunk, chunk)).expect("pack")
}

#[test]
fn query_bytes_and_distinct_keys_are_bounded_on_all_search_paths() {
    let container = fixture(b"alpha beta gamma\n", 64);
    let reader = QztReader::open(&container).expect("memory reader");
    let file = QztFileReader::open_read_at(container.as_slice(), container.len() as u64)
        .expect("file reader");
    let token = RawTokenIndex::build_from_container(&container, TokenIndexBuildOptions::default())
        .expect("token index");
    let ngram = RawNgramIndex::build_from_container(
        &container,
        NgramIndexBuildOptions {
            n: 2,
            ..Default::default()
        },
    )
    .expect("ngram index");
    let token_qzi = build_search_sidecar(&container, SidecarIndexKind::Token).expect("token qzi");
    let ngram_qzi =
        build_search_sidecar(&container, SidecarIndexKind::Ngram { n: 2 }).expect("ngram qzi");

    for (query, options) in [
        (
            "alpha",
            SearchOptions {
                max_query_bytes: 4,
                ..Default::default()
            },
        ),
        (
            "alpha beta",
            SearchOptions {
                max_query_terms: 1,
                ..Default::default()
            },
        ),
    ] {
        assert_eq!(
            token.search(&reader, query, options).map(|_| ()),
            Err(QztError::ResourceLimitExceeded)
        );
        assert_eq!(
            token.search_file(&file, query, options).map(|_| ()),
            Err(QztError::ResourceLimitExceeded)
        );
        let sidecar =
            QziFileSidecar::open_read_at(token_qzi.as_slice(), token_qzi.len() as u64, &file)
                .expect("file qzi");
        assert_eq!(
            sidecar.search(&file, query, options).map(|_| ()),
            Err(QztError::ResourceLimitExceeded)
        );
    }
    let ngram_options = SearchOptions {
        max_query_terms: 1,
        ..Default::default()
    };
    assert_eq!(
        ngram.search(&reader, "abcd", ngram_options).map(|_| ()),
        Err(QztError::ResourceLimitExceeded)
    );
    assert_eq!(
        ngram.search_file(&file, "abcd", ngram_options).map(|_| ()),
        Err(QztError::ResourceLimitExceeded)
    );
    let sidecar = QziFileSidecar::open_read_at(ngram_qzi.as_slice(), ngram_qzi.len() as u64, &file)
        .expect("ngram qzi");
    assert_eq!(
        sidecar.search(&file, "abcd", ngram_options).map(|_| ()),
        Err(QztError::ResourceLimitExceeded)
    );

    let repeated = SearchOptions {
        max_query_terms: 1,
        ..Default::default()
    };
    assert!(
        token.search(&reader, "alpha alpha", repeated).is_ok(),
        "distinct keys are counted after deduplication"
    );
}

#[test]
fn posting_bytes_ids_and_intersection_work_are_independent_of_candidate_cap() {
    let container = fixture(b"alpha\nalpha beta\nbeta\n", 64);
    let reader = QztReader::open(&container).expect("reader");
    let token = RawTokenIndex::build_from_container(&container, TokenIndexBuildOptions::default())
        .expect("index");
    for options in [
        SearchOptions {
            max_posting_bytes_per_query: 0,
            ..Default::default()
        },
        SearchOptions {
            max_posting_ids_per_query: 1,
            ..Default::default()
        },
        SearchOptions {
            max_posting_ids_per_query: 3,
            ..Default::default()
        },
        SearchOptions {
            max_posting_work: 0,
            ..Default::default()
        },
    ] {
        assert_eq!(
            token.search(&reader, "alpha beta", options).map(|_| ()),
            Err(QztError::ResourceLimitExceeded)
        );
    }
    let encoded_bytes = token
        .terms
        .iter()
        .map(|term| term.posting_size)
        .sum::<u64>();
    let decoded_ids = token
        .postings
        .iter()
        .map(|list| list.len() as u64)
        .sum::<u64>();
    let exact = SearchOptions {
        max_posting_bytes_per_query: encoded_bytes,
        max_posting_ids_per_query: decoded_ids,
        ..Default::default()
    };
    assert_eq!(
        token
            .search(&reader, "alpha beta", exact)
            .expect("exact boundary")
            .hits
            .len(),
        2
    );
    assert_eq!(
        token
            .search(
                &reader,
                "alpha beta",
                SearchOptions {
                    max_posting_bytes_per_query: encoded_bytes - 1,
                    ..exact
                }
            )
            .map(|_| ()),
        Err(QztError::ResourceLimitExceeded)
    );
    assert_eq!(
        token
            .search(
                &reader,
                "alpha beta",
                SearchOptions {
                    max_posting_ids_per_query: decoded_ids - 1,
                    ..exact
                }
            )
            .map(|_| ()),
        Err(QztError::ResourceLimitExceeded)
    );

    let qzi = build_search_sidecar(&container, SidecarIndexKind::Token).expect("qzi");
    let qzi_source = CountingReadAt::new(qzi.clone());
    let file =
        QztFileReader::open_read_at(container.as_slice(), container.len() as u64).expect("file");
    let sidecar =
        QziFileSidecar::open_read_at(qzi_source.clone(), qzi.len() as u64, &file).expect("sidecar");
    for options in [
        SearchOptions {
            max_posting_bytes_per_query: 0,
            ..Default::default()
        },
        SearchOptions {
            max_posting_ids_per_query: 1,
            ..Default::default()
        },
        SearchOptions {
            max_posting_ids_per_query: 3,
            ..Default::default()
        },
    ] {
        let reads = qzi_source.reads.lock().expect("lock").len();
        assert_eq!(
            sidecar.search(&file, "alpha beta", options).map(|_| ()),
            Err(QztError::ResourceLimitExceeded)
        );
        assert_eq!(
            qzi_source.reads.lock().expect("lock").len(),
            reads,
            "posting data was not fetched"
        );
    }
    assert_eq!(
        sidecar
            .search(&file, "alpha beta", exact)
            .expect("file exact boundary")
            .hits
            .len(),
        2
    );

    let report = token
        .search(
            &reader,
            "alpha beta",
            SearchOptions {
                max_candidate_granules: 1,
                ..Default::default()
            },
        )
        .expect("rare intersection");
    assert_eq!(report.hits.len(), 2, "rare later posting remains available");
    assert!(!report.capped);
}

#[test]
fn physical_bytes_and_chunk_count_are_charged_for_cache_misses_only() {
    let container = fixture(b"needle one\nneedle two\n", 64);
    let index = RawTokenIndex::build_from_container(&container, TokenIndexBuildOptions::default())
        .expect("index");
    let file =
        QztFileReader::open_read_at(container.as_slice(), container.len() as u64).expect("file");
    let size = file.skeleton_details().chunk_entries[0].uncompressed_size;
    let report = index
        .search_file(
            &file,
            "needle",
            SearchOptions {
                max_physical_decoded_bytes: size,
                max_physical_decoded_chunks: 1,
                ..Default::default()
            },
        )
        .expect("one cached chunk");
    assert_eq!(report.hits.len(), 2);
    assert_eq!(report.metrics.physical_decoded_bytes, size);
    assert_eq!(report.metrics.physical_decoded_chunks, 1);
    assert_eq!(report.stop_reason, None);

    let report = index
        .search_file(
            &file,
            "needle",
            SearchOptions {
                max_physical_decoded_chunks: 0,
                ..Default::default()
            },
        )
        .expect("zero chunks");
    assert_eq!(report.stop_reason, Some("max_physical_decoded_chunks"));
    assert_eq!(report.metrics.physical_decoded_chunks, 0);
}

#[test]
fn line_limit_counts_lf_crlf_unterminated_and_continuation_bytes() {
    for (input, chunk, accepted, rejected) in [
        (b"abc\n".as_slice(), 64, 4, 3),
        (b"a\r\n".as_slice(), 64, 3, 2),
        (b"abc".as_slice(), 64, 3, 2),
        (b"abcd\n".as_slice(), 2, 5, 4),
    ] {
        let container = fixture(input, chunk);
        for kind in [SidecarIndexKind::Token, SidecarIndexKind::Ngram { n: 2 }] {
            let file = QztFileReader::open_read_at(container.as_slice(), container.len() as u64)
                .expect("file");
            assert_eq!(
                build_search_sidecar_from_file_with_line_limit(&file, kind, rejected).map(|_| ()),
                Err(QztError::ResourceLimitExceeded)
            );
            build_search_sidecar_from_file_with_line_limit(&file, kind, accepted)
                .expect("boundary accepted");
        }
    }
}

#[test]
fn cli_rejects_oversized_query_before_open_or_index_build_and_reports_cap_reason() {
    let temp = tempfile::tempdir().expect("tempdir");
    let invalid = temp.path().join("invalid.qzt");
    fs::write(&invalid, b"not a qzt").expect("write");
    let oversized = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args([
            "search",
            invalid.to_str().unwrap(),
            "needle",
            "--max-query-bytes",
            "3",
        ])
        .output()
        .expect("run");
    assert_eq!(oversized.status.code(), Some(1));
    assert!(oversized.stdout.is_empty());
    assert!(String::from_utf8_lossy(&oversized.stderr).contains("resource limit"));

    let container = fixture(b"needle\n", 64);
    let path = temp.path().join("valid.qzt");
    fs::write(&path, container).expect("write");
    let capped = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args([
            "search",
            path.to_str().unwrap(),
            "needle",
            "--max-physical-decoded-chunks",
            "0",
            "--format",
            "json",
        ])
        .output()
        .expect("run");
    assert!(capped.status.success());
    let json: serde_json::Value = serde_json::from_slice(&capped.stdout).expect("json");
    assert_eq!(json["capped"], true);
    assert_eq!(json["stop_reason"], "max_physical_decoded_chunks");
    assert_eq!(json["metrics"]["physical_decoded_chunks"], 0);
    assert_eq!(json["hits"].as_array().unwrap().len(), 0);

    let zero_results = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args([
            "search",
            path.to_str().unwrap(),
            "needle",
            "--max-results",
            "0",
            "--format",
            "json",
        ])
        .output()
        .expect("run");
    assert!(zero_results.status.success());
    let json: serde_json::Value = serde_json::from_slice(&zero_results.stdout).expect("json");
    assert_eq!(json["stop_reason"], "max_search_results");
    assert_eq!(json["hits"].as_array().unwrap().len(), 0);

    let ordinary_miss = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args([
            "search",
            path.to_str().unwrap(),
            "missing",
            "--format",
            "json",
        ])
        .output()
        .expect("run");
    assert!(ordinary_miss.status.success());
    let json: serde_json::Value = serde_json::from_slice(&ordinary_miss.stdout).expect("json");
    assert_eq!(json["capped"], false);
    assert!(json["stop_reason"].is_null());

    let sidecar_path = temp.path().join("too-long.qzi");
    let line_error = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args([
            "sidecar-rebuild",
            path.to_str().unwrap(),
            "-o",
            sidecar_path.to_str().unwrap(),
            "--max-line-bytes",
            "6",
        ])
        .output()
        .expect("run");
    assert_eq!(line_error.status.code(), Some(1));
    assert!(line_error.stdout.is_empty());
    assert!(!sidecar_path.exists());
}
