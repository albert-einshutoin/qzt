#![cfg(feature = "internal-testing")]
use std::fmt::Write as _;
use std::fs;
use std::process::Command;

use qzt::chunk_table::ChunkEntry;
use qzt::error::QztError;
use qzt::reader::QztReader;
use qzt::search::{
    PostingGranularity, RawTokenIndex, SearchGranule, SearchIndexSource, SearchOptions,
    TermDictionaryEntry, TokenIndexBuildOptions, decode_delta_varint_u64, encode_delta_varint_u64,
};
use qzt::skeleton::open_skeleton_details;
use qzt::writer::pack_bytes_with_container_id;
mod support;
use support::{assert_semantic_report_eq, assert_success, output_success, writer_options};

#[test]
fn line_granules_are_inside_original_and_cover_overlapping_chunks() {
    let input = b"alpha one\nbeta two\nerror three";
    let container = pack_bytes_with_container_id(input, [0xc0; 16], writer_options(8, 8))
        .expect("container should pack");
    let details = open_skeleton_details(&container).expect("skeleton should open");
    let index = RawTokenIndex::build_from_container(&container, TokenIndexBuildOptions::default())
        .expect("raw line token index should build");

    assert_eq!(index.posting_granularity, PostingGranularity::Line);
    assert_eq!(index.granules.len(), 3);
    for (expected_id, granule) in index.granules.iter().enumerate() {
        assert_eq!(granule.granule_id, expected_id as u64);
        assert!(granule.logical_offset + granule.byte_length <= input.len() as u64);
        assert_eq!(granule.first_line, Some(expected_id as u64));
        assert_eq!(granule.line_count, Some(1));
        assert_eq!(
            (granule.chunk_start, granule.chunk_end),
            overlapping_chunk_range(
                &details.chunk_entries,
                granule.logical_offset,
                granule.byte_length
            )
        );
    }
}

#[test]
fn term_dictionary_and_postings_are_sorted() {
    let input = b"zeta alpha\nbeta alpha\n";
    let container = pack_bytes_with_container_id(input, [0xc1; 16], writer_options(64, 64))
        .expect("container should pack");
    let index = RawTokenIndex::build_from_container(&container, TokenIndexBuildOptions::default())
        .expect("raw line token index should build");

    let keys = index
        .terms
        .iter()
        .map(|term| term.key.as_slice())
        .collect::<Vec<_>>();
    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    for postings in &index.postings {
        assert!(postings.windows(2).all(|pair| pair[0] < pair[1]));
    }
}

#[test]
fn unsorted_posting_lists_are_rejected() {
    let error = RawTokenIndex::from_parts(
        [0xc5; 16],
        18,
        vec![granule(0, 0, 6), granule(1, 6, 6), granule(2, 12, 6)],
        vec![term_with_real_hash(b"alpha")],
        vec![vec![2, 1]],
    )
    .expect_err("unsorted postings must be invalid");

    assert_eq!(error, QztError::ContainerCorrupt);
}

#[test]
fn delta_varint_postings_round_trip_large_granule_ids() {
    let postings = vec![0, 127, 128, 16_384, 9_000_000_000];
    let encoded = encode_delta_varint_u64(&postings).expect("postings should encode");
    let decoded = decode_delta_varint_u64(&encoded).expect("postings should decode");

    assert_eq!(decoded, postings);
}

#[test]
fn exact_key_comparison_wins_over_key_hash_collision() {
    let mut beta_hash = [0_u8; 16];
    let hash = blake3::hash(b"beta");
    beta_hash.copy_from_slice(&hash.as_bytes()[..16]);

    let index = RawTokenIndex::from_parts(
        [0xc2; 16],
        12,
        vec![granule(0, 0, 6), granule(1, 6, 6)],
        vec![term(b"alpha", beta_hash), term(b"beta", beta_hash)],
        vec![vec![0], vec![1]],
    )
    .expect("collision fixture should be structurally valid");

    assert_eq!(index.posting_list_for_key(b"beta"), Some(&[1][..]));
}

#[test]
fn token_search_candidates_are_verified_against_original_bytes() {
    let input = b"alpha\nbeta\n";
    let container = pack_bytes_with_container_id(input, [0xc3; 16], writer_options(64, 64))
        .expect("container should pack");
    let base_index =
        RawTokenIndex::build_from_container(&container, TokenIndexBuildOptions::default())
            .expect("raw line token index should build");
    let stale_index = RawTokenIndex::from_parts(
        base_index.container_id,
        base_index.source_size_bytes,
        base_index.granules.clone(),
        vec![term_with_real_hash(b"alpha")],
        vec![vec![0, 1]],
    )
    .expect("stale candidate fixture should be structurally valid");
    let reader = QztReader::open(container).expect("reader should open");

    let report = stale_index
        .search(&reader, "alpha", SearchOptions::default())
        .expect("search should run");

    assert_eq!(report.metrics.candidate_granules, 2);
    assert_eq!(report.metrics.verified_matches, 1);
    assert_eq!(report.hits.len(), 1);
    assert_eq!(report.hits[0].logical_offset, 0);
    assert_eq!(report.hits[0].byte_length, 5);
    assert_eq!(report.hits[0].source, "verified_original_bytes");
}

#[test]
fn multi_token_search_returns_verified_hits_for_every_query_token() {
    let input = b"alpha beta\nbeta gamma\n";
    let container = pack_bytes_with_container_id(input, [0xc6; 16], writer_options(64, 64))
        .expect("container should pack");
    let index = RawTokenIndex::build_from_container(&container, TokenIndexBuildOptions::default())
        .expect("raw line token index should build");
    let reader = QztReader::open(container).expect("reader should open");

    let report = index
        .search(&reader, "alpha beta", SearchOptions::default())
        .expect("search should run");
    let hit_ranges = report
        .hits
        .iter()
        .map(|hit| (hit.logical_offset, hit.byte_length))
        .collect::<Vec<_>>();

    assert_eq!(report.metrics.verified_matches, 2);
    assert_eq!(hit_ranges, vec![(0, 5), (6, 4)]);
}

#[test]
fn normalized_token_index_is_rejected_in_phase11() {
    let input = b"alpha\n";
    let container = pack_bytes_with_container_id(input, [0xc4; 16], writer_options(64, 64))
        .expect("container should pack");
    let error = RawTokenIndex::build_from_container(
        &container,
        TokenIndexBuildOptions {
            source: SearchIndexSource::NormalizedUtf8,
            ..TokenIndexBuildOptions::default()
        },
    )
    .expect_err("normalized index is out of scope");

    assert_eq!(
        error,
        QztError::UnsupportedIndexMode("normalized_utf8 token index")
    );
}

#[test]
fn cli_search_reports_verified_hits_and_metrics() {
    let base = std::env::temp_dir().join(format!("qzt-phase11-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    fs::write(&input, b"info\nerror code\nerror again\n").expect("input should be written");

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
            .arg("error"),
    );
    let output = String::from_utf8(output).expect("search output should be utf-8");

    assert!(output.contains("source=verified_original_bytes"));
    assert!(output.contains("candidate_granules=2"));
    assert!(output.contains("candidate_chunks="));
    assert!(output.contains("decoded_bytes="));
    assert!(output.contains("query_time_ms="));
    assert!(output.contains("index_size_ratio="));

    let _ = fs::remove_dir_all(base);
}

#[test]
fn cli_search_max_candidate_granules_caps_without_decoding() {
    let base = std::env::temp_dir().join(format!(
        "qzt-phase11-max-candidate-granules-{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    let mut lines = String::new();
    for index in 0..128 {
        writeln!(lines, "aaa common {index}").expect("line should format");
    }
    fs::write(&input, lines).expect("input should be written");

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
            .arg("aaa")
            .arg("--max-candidate-granules")
            .arg("10"),
    );
    let output = String::from_utf8(output).expect("search output should be utf-8");

    assert!(output.contains("capped=true"));
    assert!(output.contains("decoded_bytes=0"));
    assert!(output.contains("incomplete_reason=none"));

    let _ = fs::remove_dir_all(base);
}

#[test]
fn cli_search_max_decoded_bytes_caps_before_decode() {
    let base = std::env::temp_dir().join(format!(
        "qzt-phase11-max-decoded-bytes-{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    fs::write(&input, b"info\nerror code\nerror again\n").expect("input should be written");

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
            .arg("error")
            .arg("--max-decoded-bytes")
            .arg("0"),
    );
    let output = String::from_utf8(output).expect("search output should be utf-8");

    assert!(output.contains("capped=true"));
    assert!(output.contains("decoded_bytes=0"));
    assert!(output.contains("incomplete_reason=none"));

    let _ = fs::remove_dir_all(base);
}

#[test]
fn cli_search_resource_limit_flags_reject_invalid_values() {
    let base = std::env::temp_dir().join(format!(
        "qzt-phase11-resource-limit-invalid-{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    fs::write(&input, b"alpha\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );

    for (flag, value) in [
        ("--max-candidate-granules", "not-a-number"),
        ("--max-decoded-bytes", "not-a-number"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("search")
            .arg(&packed)
            .arg("alpha")
            .arg(flag)
            .arg(value)
            .output()
            .expect("search command should run");
        assert_eq!(
            output.status.code(),
            Some(2),
            "{} {} must exit 2: stderr={}",
            flag,
            value,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("search")
        .arg(&packed)
        .arg("alpha")
        .arg("--max-candidate-granules")
        .output()
        .expect("search command should run");
    assert_eq!(
        output.status.code(),
        Some(2),
        "missing --max-candidate-granules value must exit 2: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(base);
}

/// Zero-hit `qzt search --format json` must emit a single parseable JSON object
/// with an empty `hits` array and explicit `incomplete_reason: null`, without
/// stderr warnings that would break JSON consumers.
#[test]
fn cli_search_json_zero_hits_emits_parseable_empty_contract() {
    let base = std::env::temp_dir().join(format!("qzt-phase11-json-zero-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    fs::write(&input, b"alpha\nbeta\ngamma\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("search")
        .arg(&packed)
        .arg("absent_token")
        .arg("--format")
        .arg("json")
        .output()
        .expect("search command should run");

    assert!(
        output.status.success(),
        "search must succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "zero-hit success must not write warnings to stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("stdout must be a single JSON object: {error}"));

    let hits = value
        .get("hits")
        .and_then(serde_json::Value::as_array)
        .expect("hits must be an array");
    assert!(hits.is_empty(), "hits must be empty: {value}");

    assert_eq!(
        value.get("incomplete_reason"),
        Some(&serde_json::Value::Null),
        "incomplete_reason must be JSON null, not omitted: {value}"
    );

    assert!(
        value
            .get("metrics")
            .and_then(serde_json::Value::as_object)
            .is_some(),
        "metrics must be an object: {value}"
    );
    assert!(
        value
            .get("capped")
            .and_then(serde_json::Value::as_bool)
            .is_some(),
        "capped must be a bool: {value}"
    );

    let _ = fs::remove_dir_all(base);
}

fn overlapping_chunk_range(entries: &[ChunkEntry], offset: u64, length: u64) -> (u64, u64) {
    let end = offset + length;
    let mut first = None;
    let mut last_exclusive = None;
    for entry in entries {
        let chunk_end = entry.logical_offset + entry.uncompressed_size;
        if chunk_end > offset && entry.logical_offset < end {
            first.get_or_insert(entry.chunk_id);
            last_exclusive = Some(entry.chunk_id + 1);
        }
    }
    (first.unwrap(), last_exclusive.unwrap())
}

fn granule(granule_id: u64, logical_offset: u64, byte_length: u64) -> SearchGranule {
    SearchGranule {
        granule_id,
        logical_offset,
        byte_length,
        chunk_start: granule_id,
        chunk_end: granule_id + 1,
        first_line: Some(granule_id),
        line_count: Some(1),
    }
}

fn term(key: &[u8], key_hash: [u8; 16]) -> TermDictionaryEntry {
    TermDictionaryEntry {
        key: key.to_vec(),
        key_hash,
        document_frequency: 0,
        granule_frequency: 0,
        posting_offset: 0,
        posting_size: 0,
        skip_offset: 0,
        skip_size: 0,
        flags: 0,
    }
}

fn term_with_real_hash(key: &[u8]) -> TermDictionaryEntry {
    let hash = blake3::hash(key);
    let mut key_hash = [0_u8; 16];
    key_hash.copy_from_slice(&hash.as_bytes()[..16]);
    term(key, key_hash)
}

#[test]
fn unindexable_query_reports_incomplete_reason() {
    let input = b"alpha one\nbeta two\n";
    let container = pack_bytes_with_container_id(input, [0xc7; 16], writer_options(64, 64))
        .expect("container should pack");
    let index = RawTokenIndex::build_from_container(&container, TokenIndexBuildOptions::default())
        .expect("raw line token index should build");
    let reader = QztReader::open(&container).expect("reader should open");

    let report = index
        .search(&reader, "証拠", SearchOptions::default())
        .expect("search should run");

    assert!(report.hits.is_empty());
    assert_eq!(
        report.incomplete_reason,
        Some("query_has_no_indexable_tokens")
    );
}

#[test]
fn dense_query_amortizes_physical_chunk_decodes() {
    let mut input = String::new();
    for index in 0..128 {
        let _ = writeln!(input, "common line {index}");
    }
    let container =
        pack_bytes_with_container_id(input.as_bytes(), [0xc8; 16], writer_options(64, 64))
            .expect("container should pack");
    let index = RawTokenIndex::build_from_container(&container, TokenIndexBuildOptions::default())
        .expect("raw line token index should build");
    let reader = QztReader::open(&container).expect("reader should open");

    let report = index
        .search(&reader, "common", SearchOptions::default())
        .expect("search should run");

    assert_eq!(report.metrics.verified_matches, 128);
    assert!(report.metrics.physical_decoded_bytes > 0);
    // Sorted candidates decode each overlapping chunk at most once; without
    // the chunk decode cache this would be candidates x chunk size and far
    // exceed the original size.
    assert!(report.metrics.physical_decoded_bytes <= reader.info().original_size);
}

#[test]
fn token_build_and_search_from_file_match_in_memory_paths() {
    let mut input = String::new();
    for index in 0..32 {
        let _ = writeln!(input, "alpha beta line {index}");
    }
    input.push_str("needle alpha\n");
    let container =
        pack_bytes_with_container_id(input.as_bytes(), [0xc9; 16], writer_options(64, 64))
            .expect("container should pack");

    let memory_index =
        RawTokenIndex::build_from_container(&container, TokenIndexBuildOptions::default())
            .expect("in-memory build should work");
    let file_reader =
        qzt::reader::QztFileReader::open_read_at(container.as_slice(), container.len() as u64)
            .expect("file reader should open");
    let file_index =
        RawTokenIndex::build_from_file(&file_reader, TokenIndexBuildOptions::default())
            .expect("file build should work");
    assert_eq!(memory_index, file_index);

    let memory_reader = QztReader::open(&container).expect("reader should open");
    for query in ["needle", "alpha beta", "absent"] {
        let memory = memory_index
            .search(&memory_reader, query, SearchOptions::default())
            .expect("in-memory search should run");
        let file = memory_index
            .search_file(&file_reader, query, SearchOptions::default())
            .expect("file search should run");
        assert_semantic_report_eq(&memory, &file, &format!("query {query:?}"));
    }
}
