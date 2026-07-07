use std::fs;
use std::io::Write as _;
use std::process::{Command, Stdio};

use qzt::skeleton::open_skeleton_details;
use qzt::{
    Checksum, ChunkerOptions, DocumentEntry, DocumentIndex, WriterOptions, pack_bytes,
    pack_bytes_with_document_index,
};
mod support;
use support::{assert_success, output_success};

#[test]
fn cli_pack_info_verify_range_lines_and_export_round_trip() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    let restored = base.join("restored.txt");
    fs::write(&input, b"alpha\nbeta\ngamma\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );
    assert!(packed.exists());

    let info = output_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("info")
            .arg(&packed),
    );
    let info = String::from_utf8(info).expect("info should be utf-8");
    assert!(info.contains("Format: QZT 0.1"));
    assert!(info.contains("Profile: core"));
    assert!(info.contains("Original size: 17"));
    assert!(info.contains("Chunks:"));
    assert!(info.contains("Lines: 3"));
    assert!(info.contains("Line index: sparse"));

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("verify")
            .arg(&packed),
    );
    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("verify")
            .arg(&packed)
            .arg("--deep"),
    );

    let line_range = output_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("range")
            .arg(&packed)
            .arg("--lines")
            .arg("2:3"),
    );
    assert_eq!(line_range, b"beta\ngamma\n");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("export")
            .arg(&packed)
            .arg("-o")
            .arg(&restored),
    );
    assert_eq!(
        fs::read(&restored).expect("restored should exist"),
        b"alpha\nbeta\ngamma\n"
    );

    let _ = fs::remove_dir_all(base);
}

#[test]
fn cli_pack_rejects_invalid_utf8() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-invalid-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("invalid.bin");
    let packed = base.join("invalid.qzt");
    fs::write(&input, [0xff]).expect("input should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("pack")
        .arg(&input)
        .arg("-o")
        .arg(&packed)
        .output()
        .expect("qzt pack should run");

    assert!(!output.status.success());
    assert!(!packed.exists());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not valid UTF-8"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(base);
}

/// CHANGELOG contract: `qzt pack -` with `--dense-line-index on` exits 2 and explains
/// the streaming-only stdin path so large streams are never buffered silently.
#[test]
fn stdin_pack_dense_line_index_conflict_exits_2_with_clear_stderr() {
    let base = std::env::temp_dir().join(format!(
        "qzt-phase9-stdin-dense-conflict-{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&base);
    let stdin_input = base.join("stdin.txt");
    let packed = base.join("never.qzt");
    fs::write(&stdin_input, b"alpha\nbeta\n").expect("stdin fixture should be written");

    let stdin_file = fs::File::open(&stdin_input).expect("stdin fixture should open");
    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("pack")
        .arg("-")
        .arg("-o")
        .arg(&packed)
        .arg("--dense-line-index")
        .arg("on")
        .stdin(Stdio::from(stdin_file))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("qzt pack should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdin + --dense-line-index on must exit 2"
    );
    assert!(
        output.stdout.is_empty(),
        "stdout must be empty on usage error, got: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("stdin"),
        "stderr must mention stdin, got: {stderr}"
    );
    assert!(
        stderr.contains("--dense-line-index"),
        "stderr must mention --dense-line-index, got: {stderr}"
    );
    assert!(
        stderr.contains("--profile core"),
        "stderr must point to --profile core, got: {stderr}"
    );
    assert!(
        stderr.contains("streaming pack path"),
        "stderr must mention streaming pack path, got: {stderr}"
    );
    assert!(
        !packed.exists(),
        "no container should be written on usage error"
    );

    let _ = fs::remove_dir_all(base);
}

/// Issue #137: `qzt pack -` on `--profile core` stdin path round-trips empty and
/// 1-byte inputs through deep verify and export.
#[test]
fn stdin_pack_empty_and_one_byte_deep_verify_and_export_roundtrip() {
    let cases: &[(&str, &[u8])] = &[("empty", b""), ("one-byte", b"x")];

    for (label, input) in cases {
        let base =
            std::env::temp_dir().join(format!("qzt-phase9-stdin-{label}-{}", std::process::id()));
        let _ = fs::create_dir_all(&base);
        let packed = base.join("stdin.qzt");

        let mut child = Command::new(env!("CARGO_BIN_EXE_qzt"))
            .args(["pack", "-", "-o", packed.to_str().unwrap()])
            .stdin(Stdio::piped())
            .spawn()
            .expect("qzt pack should spawn");
        if !input.is_empty() {
            child
                .stdin
                .as_mut()
                .expect("stdin pipe should exist")
                .write_all(input)
                .expect("stdin bytes should be written");
        }
        // Close stdin so `qzt pack` receives EOF and can finish.
        drop(child.stdin.take());
        let status = child.wait().expect("qzt pack should finish");
        assert!(
            status.success(),
            "{label}: qzt pack - must succeed, got: {status:?}"
        );
        assert!(packed.exists(), "{label}: packed container must exist");

        assert_success(
            Command::new(env!("CARGO_BIN_EXE_qzt"))
                .arg("verify")
                .arg(&packed)
                .arg("--deep"),
        );

        let exported = output_success(
            Command::new(env!("CARGO_BIN_EXE_qzt"))
                .arg("export")
                .arg(&packed),
        );
        assert_eq!(
            exported, *input,
            "{label}: export must restore original stdin bytes"
        );

        let _ = fs::remove_dir_all(&base);
    }
}

#[test]
fn cli_pack_profile_dense_and_writer_options_reach_metadata_and_info() {
    // The "memory" profile requires a DocumentIndex which is not expressible via
    // the CLI pack command. Use "archive" to verify that profile, dense-line-index,
    // and writer options are correctly forwarded to the container metadata.
    let base = std::env::temp_dir().join(format!("qzt-phase9-profile-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    fs::write(&input, b"alpha\nbeta\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed)
            .arg("--profile")
            .arg("archive")
            .arg("--dense-line-index")
            .arg("on")
            .arg("--chunk-size")
            .arg("8")
            .arg("--max-chunk-size")
            .arg("8")
            .arg("--zstd-level")
            .arg("3"),
    );

    let container = fs::read(&packed).expect("packed file should exist");
    let details = open_skeleton_details(&container).expect("container should open");
    assert_eq!(details.metadata.profile, "archive");
    assert!(details.metadata.dense_line_index);
    assert_eq!(details.metadata.zstd_level, 3);
    assert_eq!(details.metadata.target_chunk_size, 8);
    assert_eq!(details.metadata.max_chunk_size, 8);
    assert!(details.dense_line_index.is_some());

    let info = output_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("info")
            .arg(&packed),
    );
    let info = String::from_utf8(info).expect("info should be utf-8");
    assert!(info.contains("Profile: archive"));
    assert!(info.contains("Zstd level: 3"));
    assert!(info.contains("Target chunk size: 8"));
    assert!(info.contains("Max chunk size: 8"));
    assert!(info.contains("Line index: sparse+dense"));

    let _ = fs::remove_dir_all(base);
}

/// `qzt info --format json` emits `container_id`, `original_checksum`, and `blake3`.
#[test]
fn info_json_contains_container_id_and_checksum() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-json-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    fs::write(&input, b"hello\nworld\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );

    let json_bytes = output_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("info")
            .arg(&packed)
            .arg("--format")
            .arg("json"),
    );
    let json = String::from_utf8(json_bytes).expect("json output should be utf-8");

    // Must be a JSON object (outer braces).
    assert!(
        json.trim_start().starts_with('{'),
        "must start with {{: {json}"
    );
    assert!(json.trim_end().ends_with('}'), "must end with }}: {json}");

    // Must contain the required identity and checksum fields.
    assert!(
        json.contains("\"container_id\""),
        "must contain container_id: {json}"
    );
    assert!(
        json.contains("\"original_checksum\""),
        "must contain original_checksum: {json}"
    );
    assert!(
        json.contains("blake3"),
        "must contain blake3 algorithm: {json}"
    );

    // Verify that --format text output is unchanged (existing behavior).
    let text_bytes = output_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("info")
            .arg(&packed),
    );
    let text = String::from_utf8(text_bytes).expect("text output should be utf-8");
    assert!(text.contains("Format: QZT 0.1"));
    assert!(
        text.contains("Container ID:"),
        "text mode must show Container ID: {text}"
    );
    assert!(
        text.contains("Original checksum:"),
        "text mode must show Original checksum: {text}"
    );
    assert!(
        text.contains("Newline mode:"),
        "text mode must show Newline mode: {text}"
    );

    let _ = fs::remove_dir_all(base);
}

/// `qzt info --format text` is accepted as explicit text mode.
#[test]
fn info_format_text_explicit_accepted() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-fmt-text-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    fs::write(&input, b"line1\nline2\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );

    let out = output_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("info")
            .arg(&packed)
            .arg("--format")
            .arg("text"),
    );
    let out = String::from_utf8(out).expect("output should be utf-8");
    assert!(out.contains("Format: QZT 0.1"));

    let _ = fs::remove_dir_all(base);
}

/// An unknown `--format` value must exit with code 2.
#[test]
fn info_unknown_format_exits_2() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-fmt-bad-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    fs::write(&input, b"a\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("info")
        .arg(&packed)
        .arg("--format")
        .arg("yaml")
        .output()
        .expect("command should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown --format value must exit 2"
    );

    let _ = fs::remove_dir_all(base);
}

/// `qzt verify --format json` on a valid container reports `"ok":true` and a non-zero
/// `checked_chunks` count.
#[test]
fn verify_json_reports_ok_with_counts() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-vjson-ok-{}", std::process::id()));
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

    let json_bytes = output_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("verify")
            .arg(&packed)
            .arg("--format")
            .arg("json"),
    );
    let json = String::from_utf8(json_bytes).expect("json output should be utf-8");

    assert!(json.contains("\"ok\":true"), "must contain ok:true: {json}");
    assert!(
        json.contains("\"level\""),
        "must contain level field: {json}"
    );
    // checked_chunks must be at least 1.
    assert!(
        json.contains("\"checked_chunks\":") && !json.contains("\"checked_chunks\":0"),
        "checked_chunks must be >= 1: {json}"
    );
    // decoded_bytes is part of the frozen CLI contract.
    assert!(
        json.contains("\"decoded_bytes\":"),
        "must contain decoded_bytes field: {json}"
    );

    let _ = fs::remove_dir_all(base);
}

/// `qzt verify --deep --format json` on a corrupt container exits with code 1 and emits
/// `"ok":false` plus an `"error"` key to stdout (no stderr output in JSON mode).
#[test]
fn verify_json_reports_failure_with_exit_1() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-vjson-fail-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    let corrupt = base.join("corrupt.qzt");
    fs::write(&input, b"alpha\nbeta\ngamma\ndelta\nepsilon\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );

    // Flip a byte near the middle of the container to corrupt a chunk payload.
    let mut bytes = fs::read(&packed).expect("packed container should be readable");
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xff;
    fs::write(&corrupt, &bytes).expect("corrupt file should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("verify")
        .arg(&corrupt)
        .arg("--deep")
        .arg("--format")
        .arg("json")
        .output()
        .expect("command should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "corrupt container must exit 1"
    );

    // In JSON mode all output goes to stdout; stderr must be empty.
    assert!(
        output.stderr.is_empty(),
        "stderr must be empty in JSON mode, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json = String::from_utf8(output.stdout).expect("json output should be utf-8");
    assert!(
        json.contains("\"ok\":false"),
        "must contain ok:false: {json}"
    );
    assert!(
        json.contains("\"error\""),
        "must contain error field: {json}"
    );

    let _ = fs::remove_dir_all(base);
}

/// `qzt verify --deep` text mode prints `Decoded bytes:` matching the original size.
#[test]
fn verify_deep_text_reports_decoded_bytes() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-vdeep-text-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    let content = b"alpha\nbeta\ngamma\n";
    fs::write(&input, content).expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );

    let out_bytes = output_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("verify")
            .arg(&packed)
            .arg("--deep"),
    );
    let out = String::from_utf8(out_bytes).expect("output should be utf-8");

    // The first line must remain byte-identical to the pre-existing format.
    assert!(
        out.starts_with("Verify: Deep ok\n"),
        "first line must be 'Verify: Deep ok': {out:?}"
    );

    // Decoded bytes must equal the original content size.
    let expected = format!("Decoded bytes: {}", content.len());
    assert!(out.contains(&expected), "must contain '{expected}': {out}");

    let _ = fs::remove_dir_all(base);
}

/// `qzt info <file> --format` with the flag as the last argument (missing value) exits 2.
#[test]
fn info_format_missing_value_exits_2() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-fmt-missing-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    fs::write(&input, b"a\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("info")
        .arg(&packed)
        .arg("--format")
        .output()
        .expect("command should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "--format with missing value must exit 2"
    );

    let _ = fs::remove_dir_all(base);
}

/// `qzt verify --format bad` (unknown format value) exits with code 2.
#[test]
fn verify_unknown_format_exits_2() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-vfmt-bad-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    fs::write(&input, b"a\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("verify")
        .arg(&packed)
        .arg("--format")
        .arg("yaml")
        .output()
        .expect("command should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown --format value must exit 2"
    );

    let _ = fs::remove_dir_all(base);
}

/// `qzt line <file> 0` is a usage error (CLI line numbers are 1-based); `1` reads the first line.
#[test]
fn cli_line_rejects_zero_and_reads_first_with_one() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-line-{}", std::process::id()));
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

    let zero = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("line")
        .arg(&packed)
        .arg("0")
        .output()
        .expect("qzt line 0 should run");

    assert_eq!(
        zero.status.code(),
        Some(2),
        "qzt line 0 must exit 2 (1-based CLI line numbers)"
    );
    let stderr = String::from_utf8_lossy(&zero.stderr);
    assert!(
        stderr.contains("1-based"),
        "stderr must explain 1-based line numbers: {stderr}"
    );

    let first = output_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("line")
            .arg(&packed)
            .arg("1"),
    );
    assert_eq!(first, b"alpha\n");

    let _ = fs::remove_dir_all(base);
}

/// `qzt verify --format` with the flag as the last argument (missing value) exits 2.
#[test]
fn verify_format_missing_value_exits_2() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-vfmt-missing-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    fs::write(&input, b"a\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("verify")
        .arg(&packed)
        .arg("--format")
        .output()
        .expect("command should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "--format with missing value must exit 2"
    );

    let _ = fs::remove_dir_all(base);
}

// ---------------------------------------------------------------------------
// Issue #95: focused docs/doc failure contracts (default-features CLI tests)
// ---------------------------------------------------------------------------

fn docs_doc_writer_options() -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size: 9,
            max_chunk_size: 9,
        },
        zstd_level: 0,
    }
}

const DOCS_DOC_TWO_LINES: &[u8] = b"aaaaaaaa\nbbbbbbbb\n";

fn docs_doc_indexed_container() -> Vec<u8> {
    let doc_one = DocumentEntry::new(
        "doc-one",
        0,
        9,
        0,
        1,
        0,
        1,
        Checksum::blake3(&DOCS_DOC_TWO_LINES[0..9]),
    );
    let doc_two = DocumentEntry::new(
        "doc-two",
        9,
        9,
        1,
        1,
        1,
        2,
        Checksum::blake3(&DOCS_DOC_TWO_LINES[9..18]),
    );
    let document_index = DocumentIndex {
        container_id: [0x95; 16],
        documents: vec![doc_one, doc_two],
    };
    pack_bytes_with_document_index(
        DOCS_DOC_TWO_LINES,
        [0x95; 16],
        docs_doc_writer_options(),
        &document_index,
    )
    .expect("indexed container should pack")
}

fn docs_doc_no_index_container() -> Vec<u8> {
    pack_bytes(b"hello\nworld\n", WriterOptions::default()).expect("no-index container should pack")
}

fn run_qzt(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(args)
        .output()
        .expect("qzt command should run")
}

/// CHANGELOG contract: `qzt docs` exits 1 when the container has no Document Index.
#[test]
fn docs_no_document_index_exits_1() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-docs-noindex-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let qzt_path = base.join("noidx.qzt");
    fs::write(&qzt_path, docs_doc_no_index_container()).expect("write fixture");

    let out = run_qzt(&["docs", qzt_path.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "qzt docs must exit 1 without Document Index; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.stderr.is_empty(),
        "stderr must describe the missing Document Index"
    );

    let _ = fs::remove_dir_all(base);
}

/// CHANGELOG contract: `qzt doc` exits 1 for an unknown `doc_id`.
#[test]
fn doc_unknown_id_exits_1() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-doc-unknown-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let qzt_path = base.join("indexed.qzt");
    fs::write(&qzt_path, docs_doc_indexed_container()).expect("write fixture");

    let out = run_qzt(&["doc", qzt_path.to_str().unwrap(), "unknown-doc-id"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "qzt doc must exit 1 for unknown doc_id; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.stderr.is_empty(),
        "stderr must describe the unknown doc_id"
    );

    let _ = fs::remove_dir_all(base);
}

/// CHANGELOG contract: wrong `DocumentEntry.checksum` fails verified extraction (exit 1)
/// while `--no-verify` still returns intact payload bytes.
#[test]
fn doc_tampered_entry_checksum_verified_exits_1_no_verify_succeeds() {
    let base = std::env::temp_dir().join(format!(
        "qzt-phase9-doc-tampered-chk-{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&base);

    let wrong_checksum = Checksum {
        algorithm: String::from("blake3"),
        value: [0u8; 32],
    };
    let doc_entry = DocumentEntry::new(
        "target",
        0,
        DOCS_DOC_TWO_LINES.len() as u64,
        0,
        2,
        0,
        1,
        wrong_checksum,
    );
    let document_index = DocumentIndex {
        container_id: [0x96; 16],
        documents: vec![doc_entry],
    };
    let container = pack_bytes_with_document_index(
        DOCS_DOC_TWO_LINES,
        [0x96; 16],
        docs_doc_writer_options(),
        &document_index,
    )
    .expect("tampered checksum container should pack");

    let qzt_path = base.join("tampered.qzt");
    fs::write(&qzt_path, &container).expect("write fixture");
    let path = qzt_path.to_str().unwrap();

    let verified = run_qzt(&["doc", path, "target"]);
    assert_eq!(
        verified.status.code(),
        Some(1),
        "verified extraction must exit 1 on checksum mismatch; stderr: {}",
        String::from_utf8_lossy(&verified.stderr)
    );

    let no_verify = run_qzt(&["doc", path, "target", "--no-verify"]);
    assert_eq!(
        no_verify.status.code(),
        Some(0),
        "--no-verify must exit 0 when chunk payload is intact; stderr: {}",
        String::from_utf8_lossy(&no_verify.stderr)
    );
    assert_eq!(
        no_verify.stdout, DOCS_DOC_TWO_LINES,
        "--no-verify must return the original bytes unchanged"
    );

    let _ = fs::remove_dir_all(base);
}

/// Regression guard: success path for `qzt docs --format json` remains stable.
#[test]
fn docs_json_success_path_smoke() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-docs-json-ok-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let qzt_path = base.join("indexed.qzt");
    fs::write(&qzt_path, docs_doc_indexed_container()).expect("write fixture");

    let out = run_qzt(&["docs", qzt_path.to_str().unwrap(), "--format", "json"]);
    assert!(
        out.status.success(),
        "qzt docs --format json must succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json = String::from_utf8(out.stdout).expect("stdout should be utf-8");
    assert!(
        json.trim_start().starts_with('{'),
        "must emit JSON object: {json}"
    );
    assert!(
        json.contains("\"documents\""),
        "must contain documents key: {json}"
    );
    assert!(json.contains("\"doc-one\""), "must list doc-one: {json}");

    let _ = fs::remove_dir_all(base);
}

/// Regression guard: success path for `qzt doc` verified extraction remains stable.
#[test]
fn doc_success_path_smoke() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-doc-ok-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let qzt_path = base.join("indexed.qzt");
    fs::write(&qzt_path, docs_doc_indexed_container()).expect("write fixture");

    let out = run_qzt(&["doc", qzt_path.to_str().unwrap(), "doc-one"]);
    assert!(
        out.status.success(),
        "qzt doc must succeed for known doc_id; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"aaaaaaaa\n");

    let _ = fs::remove_dir_all(base);
}
