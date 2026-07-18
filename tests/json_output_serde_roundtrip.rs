/// Serde-based validity tests for CLI `--format json` stdout (issue #89).
///
/// Each command emits hand-built JSON; these tests parse stdout with
/// `serde_json::from_str` and assert minimal required field presence.
/// Success paths must keep warnings and errors off stdout (stderr may be
/// non-empty only where existing contracts allow it — not duplicated here).
use std::fs;
use std::process::Command;

use qzt::{Checksum, ChunkerOptions, DocumentEntry, DocumentIndex, WriterBuilder, WriterOptions};

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(args)
        .output()
        .expect("command should run")
}

fn pack_simple(base: &std::path::Path) -> std::path::PathBuf {
    let input_path = base.join("input.txt");
    let packed_path = base.join("input.qzt");
    fs::write(&input_path, b"alpha\nbeta\ngamma\n").expect("input write");
    let out = run(&[
        "pack",
        input_path.to_str().unwrap(),
        "-o",
        packed_path.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "pack failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    packed_path
}

fn pack_with_document_index(base: &std::path::Path) -> std::path::PathBuf {
    const PAYLOAD: &[u8] = b"aaaaaaaa\nbbbbbbbb\n";
    let doc_one = DocumentEntry::new(
        "doc-one",
        0,
        9,
        0,
        1,
        0,
        1,
        Checksum::blake3(&PAYLOAD[0..9]),
    );
    let document_index = DocumentIndex {
        container_id: [0xd5; 16],
        documents: vec![doc_one],
    };
    let container = WriterBuilder::new()
        .container_id([0xd5; 16])
        .options(WriterOptions {
            chunker: ChunkerOptions {
                target_chunk_size: 9,
                max_chunk_size: 9,
            },
            zstd_level: 0,
        })
        .document_index(document_index)
        .pack(PAYLOAD)
        .expect("pack with document index");
    let packed_path = base.join("indexed.qzt");
    fs::write(&packed_path, container).expect("write indexed container");
    packed_path
}

fn parse_json_stdout(output: &std::process::Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "command failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "success JSON mode must not write to stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout.clone()).expect("stdout is utf-8");
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("stdout must be valid JSON: {error}\n{text}"))
}

#[test]
fn info_json_serde_roundtrip_has_required_fields() {
    let base = std::env::temp_dir().join(format!("qzt-89-info-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let packed = pack_simple(&base);
    let path = packed.to_str().unwrap();

    let value = parse_json_stdout(&run(&["info", path, "--format", "json"]));

    assert!(value.get("container_id").is_some());
    let checksum = value
        .get("original_checksum")
        .and_then(serde_json::Value::as_object)
        .expect("original_checksum must be an object");
    assert!(checksum.contains_key("algorithm"));
    assert!(checksum.contains_key("value"));
    assert_eq!(
        checksum
            .get("algorithm")
            .and_then(serde_json::Value::as_str),
        Some("blake3")
    );

    let _ = fs::remove_dir_all(base);
}

#[test]
fn verify_json_serde_roundtrip_has_required_fields() {
    let base = std::env::temp_dir().join(format!("qzt-89-verify-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let packed = pack_simple(&base);
    let path = packed.to_str().unwrap();

    let value = parse_json_stdout(&run(&["verify", path, "--format", "json"]));

    assert_eq!(
        value.get("ok").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(value.get("level").is_some());
    let checked_chunks = value
        .get("checked_chunks")
        .and_then(serde_json::Value::as_u64)
        .expect("checked_chunks must be a non-negative integer");
    assert!(checked_chunks >= 1);
    assert!(value.get("decoded_bytes").is_some());

    let _ = fs::remove_dir_all(base);
}

#[test]
fn verify_json_error_contract_is_stable() {
    let base = std::env::temp_dir().join(format!("qzt-123-verify-error-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let packed = pack_simple(&base);
    let corrupt = base.join("corrupt.qzt");

    let mut bytes = fs::read(&packed).expect("packed container should be readable");
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xff;
    fs::write(&corrupt, bytes).expect("corrupt container write");

    let output = run(&[
        "verify",
        corrupt.to_str().unwrap(),
        "--deep",
        "--format",
        "json",
    ]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stderr.is_empty(),
        "failure JSON mode must keep stderr empty: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json = String::from_utf8(output.stdout).expect("stdout is utf-8");
    let value: serde_json::Value = serde_json::from_str(&json)
        .unwrap_or_else(|error| panic!("failure stdout must be valid JSON: {error}\n{json}"));
    assert_eq!(
        value.get("ok").and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value.get("level").and_then(serde_json::Value::as_str),
        Some("deep")
    );
    let error = value
        .get("error")
        .and_then(serde_json::Value::as_str)
        .expect("error must be a string");
    assert!(!error.is_empty(), "error must not be empty");

    let _ = fs::remove_dir_all(base);
}

#[test]
fn docs_json_serde_roundtrip_has_required_fields() {
    let base = std::env::temp_dir().join(format!("qzt-89-docs-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let packed = pack_with_document_index(&base);
    let path = packed.to_str().unwrap();

    let value = parse_json_stdout(&run(&["docs", path, "--format", "json"]));

    let documents = value
        .get("documents")
        .and_then(serde_json::Value::as_array)
        .expect("documents must be an array");
    assert!(!documents.is_empty());
    let first = documents
        .first()
        .and_then(serde_json::Value::as_object)
        .expect("document entry must be an object");
    assert!(first.contains_key("doc_id"));
    assert!(first.contains_key("logical_offset"));
    assert!(first.contains_key("byte_length"));
    assert!(first.contains_key("first_line"));
    assert!(first.contains_key("line_count"));
    let checksum = first
        .get("checksum")
        .and_then(serde_json::Value::as_object)
        .expect("checksum must be an object");
    assert!(checksum.contains_key("algorithm"));
    assert!(checksum.contains_key("value"));

    let _ = fs::remove_dir_all(base);
}

#[test]
fn search_json_serde_roundtrip_has_required_fields() {
    let base = std::env::temp_dir().join(format!("qzt-89-search-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let packed = pack_simple(&base);
    let path = packed.to_str().unwrap();

    let value = parse_json_stdout(&run(&["search", path, "beta", "--format", "json"]));

    assert!(
        value
            .get("hits")
            .and_then(serde_json::Value::as_array)
            .is_some()
    );
    let metrics = value
        .get("metrics")
        .and_then(serde_json::Value::as_object)
        .expect("metrics must be an object");
    assert!(metrics.contains_key("query"));
    assert!(metrics.contains_key("index_kind"));
    assert!(value.get("capped").is_some());
    assert!(value.get("incomplete_reason").is_some());

    let _ = fs::remove_dir_all(base);
}
