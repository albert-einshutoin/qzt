use std::fs;
use std::process::Command;

use qzt::skeleton::open_skeleton_details;

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
