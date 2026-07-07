/// JSON stdout roundtrip tests for `info`, `verify`, `search`, and `docs` (issue #104).
///
/// Each command's `--format json` stdout must parse as JSON with stable required
/// field types. stderr warnings must not corrupt stdout JSON.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use qzt::{
    Checksum, ChunkerOptions, DocumentEntry, DocumentIndex, WriterOptions,
    pack_bytes_with_container_id, pack_bytes_with_document_index,
};

// ---------------------------------------------------------------------------
// Deterministic fixture constants
// ---------------------------------------------------------------------------

const PLAIN_PAYLOAD: &[u8] = b"alpha\nbeta\ngamma\n";
const PLAIN_CONTAINER_ID: [u8; 16] = [0xa4; 16];

const DOC_PAYLOAD: &[u8] = b"aaaaaaaa\nbbbbbbbb\n";
const DOC_CONTAINER_ID: [u8; 16] = [0xd5; 16];

fn writer_options(target_chunk_size: usize, max_chunk_size: usize) -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size,
            max_chunk_size,
        },
        zstd_level: 0,
    }
}

// ---------------------------------------------------------------------------
// Temp workspace (removed on drop — nothing left in repo)
// ---------------------------------------------------------------------------

struct TempWorkspace {
    path: PathBuf,
}

impl TempWorkspace {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("qzt-104-{label}-{}", std::process::id()));
        fs::create_dir_all(&path).expect("temp dir create");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

// ---------------------------------------------------------------------------
// Deterministic container writers
// ---------------------------------------------------------------------------

fn write_plain_container(dir: &Path) -> PathBuf {
    let container =
        pack_bytes_with_container_id(PLAIN_PAYLOAD, PLAIN_CONTAINER_ID, writer_options(64, 64))
            .expect("pack plain container");
    let path = dir.join("plain.qzt");
    fs::write(&path, container).expect("write plain container");
    path
}

fn write_document_index_container(dir: &Path) -> PathBuf {
    let doc_one = DocumentEntry::new(
        "doc-one",
        0,
        9,
        0,
        1,
        0,
        1,
        Checksum::blake3(&DOC_PAYLOAD[0..9]),
    );
    let document_index = DocumentIndex {
        container_id: DOC_CONTAINER_ID,
        documents: vec![doc_one],
    };
    let container = pack_bytes_with_document_index(
        DOC_PAYLOAD,
        DOC_CONTAINER_ID,
        writer_options(9, 9),
        &document_index,
    )
    .expect("pack document-index container");
    let path = dir.join("indexed.qzt");
    fs::write(&path, container).expect("write indexed container");
    path
}

// ---------------------------------------------------------------------------
// CLI + JSON helpers
// ---------------------------------------------------------------------------

fn run_qzt(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(args)
        .output()
        .expect("command should run")
}

fn parse_success_json_stdout(output: &std::process::Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "command failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be valid JSON: {error}\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn expect_str<'a>(value: &'a serde_json::Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("{key} must be a string: {value}"))
}

fn expect_u64(value: &serde_json::Value, key: &str) -> u64 {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| panic!("{key} must be a non-negative integer: {value}"))
}

fn expect_bool(value: &serde_json::Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or_else(|| panic!("{key} must be a boolean: {value}"))
}

fn expect_array<'a>(value: &'a serde_json::Value, key: &str) -> &'a Vec<serde_json::Value> {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{key} must be an array: {value}"))
}

fn expect_object<'a>(
    value: &'a serde_json::Value,
    key: &str,
) -> &'a serde_json::Map<String, serde_json::Value> {
    value
        .get(key)
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("{key} must be an object: {value}"))
}

fn expect_f64(value: &serde_json::Value, key: &str) -> f64 {
    value
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or_else(|| panic!("{key} must be a number: {value}"))
}

fn expect_checksum_object(value: &serde_json::Value, key: &str) {
    let checksum = expect_object(value, key);
    assert!(
        checksum
            .get("algorithm")
            .and_then(serde_json::Value::as_str)
            .is_some(),
        "checksum.algorithm must be a string: {value}"
    );
    assert!(
        checksum
            .get("value")
            .and_then(serde_json::Value::as_str)
            .is_some(),
        "checksum.value must be a string: {value}"
    );
}

fn assert_info_field_types(value: &serde_json::Value) {
    expect_str(value, "format");
    expect_str(value, "container_id");
    expect_str(value, "profile");
    expect_u64(value, "original_size");
    expect_u64(value, "compressed_size");
    expect_checksum_object(value, "original_checksum");
    expect_str(value, "newline_mode");
    expect_u64(value, "chunk_count");
    expect_u64(value, "line_count");
    expect_u64(value, "zstd_level");
    expect_u64(value, "target_chunk_size");
    expect_u64(value, "max_chunk_size");
    expect_bool(value, "dense_line_index");
    expect_bool(value, "document_index");
    expect_u64(value, "document_count");
}

fn assert_verify_field_types(value: &serde_json::Value) {
    assert!(expect_bool(value, "ok"));
    expect_str(value, "level");
    let checked_chunks = expect_u64(value, "checked_chunks");
    assert!(checked_chunks >= 1, "checked_chunks must be >= 1: {value}");
    expect_u64(value, "decoded_bytes");
}

fn assert_search_hit_types(hit: &serde_json::Value) {
    expect_u64(hit, "logical_offset");
    expect_u64(hit, "byte_length");
    expect_u64(hit, "chunk_start");
    expect_u64(hit, "chunk_end");
    expect_str(hit, "source");
}

fn assert_search_metrics_types(metrics: &serde_json::Map<String, serde_json::Value>) {
    let metrics_value = serde_json::Value::Object(metrics.clone());
    expect_str(&metrics_value, "query");
    expect_str(&metrics_value, "index_kind");
    expect_str(&metrics_value, "posting_granularity");
    expect_u64(&metrics_value, "index_size_bytes");
    expect_u64(&metrics_value, "source_size_bytes");
    expect_f64(&metrics_value, "index_size_ratio");
    expect_u64(&metrics_value, "term_lookups");
    expect_u64(&metrics_value, "posting_bytes_read");
    expect_u64(&metrics_value, "candidate_granules");
    expect_u64(&metrics_value, "candidate_chunks");
    expect_u64(&metrics_value, "decoded_bytes");
    expect_u64(&metrics_value, "physical_decoded_bytes");
    expect_u64(&metrics_value, "verified_matches");
    expect_f64(&metrics_value, "query_time_ms");
}

fn assert_search_field_types(value: &serde_json::Value) {
    let hits = expect_array(value, "hits");
    for hit in hits {
        assert_search_hit_types(hit);
    }
    let metrics = expect_object(value, "metrics");
    assert_search_metrics_types(metrics);
    expect_bool(value, "capped");
    let incomplete = value
        .get("incomplete_reason")
        .expect("incomplete_reason must be present");
    assert!(
        incomplete.is_null() || incomplete.is_string(),
        "incomplete_reason must be null or string: {value}"
    );
}

fn assert_docs_document_types(doc: &serde_json::Value) {
    expect_str(doc, "doc_id");
    expect_u64(doc, "logical_offset");
    expect_u64(doc, "byte_length");
    expect_u64(doc, "first_line");
    expect_u64(doc, "line_count");
    expect_checksum_object(doc, "checksum");
}

fn assert_docs_field_types(value: &serde_json::Value) {
    let documents = expect_array(value, "documents");
    assert!(
        !documents.is_empty(),
        "documents must not be empty: {value}"
    );
    for doc in documents {
        assert_docs_document_types(doc);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn info_json_stdout_roundtrips_with_stable_field_types() {
    let ws = TempWorkspace::new("info");
    let packed = write_plain_container(ws.path());
    let path = packed.to_str().unwrap();

    let value = parse_success_json_stdout(&run_qzt(&["info", path, "--format", "json"]));
    assert_info_field_types(&value);
}

#[test]
fn verify_json_stdout_roundtrips_with_stable_field_types() {
    let ws = TempWorkspace::new("verify");
    let packed = write_plain_container(ws.path());
    let path = packed.to_str().unwrap();

    let value = parse_success_json_stdout(&run_qzt(&["verify", path, "--format", "json"]));
    assert_verify_field_types(&value);
}

#[test]
fn search_json_stdout_roundtrips_with_stable_field_types() {
    let ws = TempWorkspace::new("search");
    let packed = write_plain_container(ws.path());
    let path = packed.to_str().unwrap();

    let value = parse_success_json_stdout(&run_qzt(&["search", path, "beta", "--format", "json"]));
    assert_search_field_types(&value);
}

#[test]
fn docs_json_stdout_roundtrips_with_stable_field_types() {
    let ws = TempWorkspace::new("docs");
    let packed = write_document_index_container(ws.path());
    let path = packed.to_str().unwrap();

    let value = parse_success_json_stdout(&run_qzt(&["docs", path, "--format", "json"]));
    assert_docs_field_types(&value);
}

#[test]
fn search_json_stdout_stays_parseable_when_warning_is_on_stderr() {
    let ws = TempWorkspace::new("search-warn");
    let packed = write_plain_container(ws.path());
    let path = packed.to_str().unwrap();

    // Short query against n-gram index triggers incomplete_reason on stderr.
    let output = run_qzt(&[
        "search", path, "e", "--index", "ngram", "--ngram", "3", "--format", "json",
    ]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning: result may be incomplete"),
        "stderr must contain warning: {stderr}"
    );

    let value = parse_success_json_stdout(&output);
    assert_search_field_types(&value);

    let incomplete = value
        .get("incomplete_reason")
        .and_then(serde_json::Value::as_str)
        .expect("incomplete_reason must be a string when warning is emitted");
    assert_eq!(incomplete, "query_shorter_than_ngram_n");

    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("warning"),
        "warning must not appear in stdout JSON"
    );
}
