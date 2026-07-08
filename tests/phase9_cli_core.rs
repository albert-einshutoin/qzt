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

/// Issue #91: `qzt --help` documents exit codes 0, 1, 2 for automation contracts.
#[test]
fn help_mentions_exit_codes() {
    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("--help")
        .output()
        .expect("qzt --help should run");

    assert!(
        output.status.success(),
        "qzt --help must exit 0, got {:?}",
        output.status.code()
    );

    let stdout = String::from_utf8(output.stdout).expect("help output should be UTF-8");
    assert!(
        stdout.contains("Exit codes:"),
        "help must document exit codes section, got:\n{stdout}"
    );
    assert!(
        stdout.contains("0  success (verify: container is valid)"),
        "help must document exit code 0, got:\n{stdout}"
    );
    assert!(
        stdout.contains("1  command failed (verify: container is corrupt or unreadable)"),
        "help must document exit code 1, got:\n{stdout}"
    );
    assert!(
        stdout.contains("2  usage error (unknown option / missing argument)"),
        "help must document exit code 2, got:\n{stdout}"
    );
}

/// Issue #152: `qzt pack --help` documents stdin packing constraints near I/O usage.
#[test]
fn pack_help_mentions_stdin_packing_constraints() {
    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(["pack", "--help"])
        .output()
        .expect("qzt pack --help should run");

    assert!(
        output.status.success(),
        "pack --help must exit 0, got {:?}",
        output.status.code()
    );

    let stdout = String::from_utf8(output.stdout).expect("help output should be UTF-8");
    assert!(
        stdout.contains("stdin"),
        "pack --help must mention stdin, got:\n{stdout}"
    );
    assert!(
        stdout.contains("--profile core"),
        "pack --help must mention --profile core, got:\n{stdout}"
    );
    assert!(
        stdout.contains("--dense-line-index"),
        "pack --help must mention --dense-line-index, got:\n{stdout}"
    );
    assert!(
        stdout.contains("-o <path>"),
        "pack --help must mention -o <path>, got:\n{stdout}"
    );
    assert!(
        stdout.contains("stdout output is not supported"),
        "pack --help must state stdout output is unsupported, got:\n{stdout}"
    );
}

fn run_stdin_pack(args: &[&str], stdin_bytes: &[u8]) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_qzt"));
    cmd.arg("pack").arg("-");
    for arg in args {
        cmd.arg(arg);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("qzt pack should spawn");
    if !stdin_bytes.is_empty() {
        if let Some(stdin) = child.stdin.as_mut() {
            // Validation may exit before stdin is fully consumed; BrokenPipe is expected then.
            let _ = stdin.write_all(stdin_bytes);
        }
    }
    // Close stdin so `qzt pack` receives EOF and can finish.
    drop(child.stdin.take());
    child.wait_with_output().expect("qzt pack should finish")
}

struct StdinPackRejectionCase {
    name: &'static str,
    extra_pack_args: &'static [&'static str],
    stderr_needles: &'static [&'static str],
    with_output_path: bool,
}

/// Issue #81: `qzt pack -` with `--dense-line-index on` is rejected on the streaming path.
#[test]
fn stdin_dense_line_index_rejected() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-stdin-dli-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let packed = base.join("stdin-dli.qzt");
    let packed_str = packed.to_str().expect("output path is utf-8");

    let output = run_stdin_pack(
        &["-o", packed_str, "--dense-line-index", "on"],
        b"alpha\nbeta\n",
    );

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdin pack with --dense-line-index on must exit 2"
    );
    assert!(
        output.stdout.is_empty(),
        "stdout must be empty on usage error, got: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    for needle in [
        "stdin",
        "--dense-line-index on",
        "Dense Line Index",
        "--profile core",
        "streaming pack path",
    ] {
        assert!(
            stderr.contains(needle),
            "stderr must contain {needle:?}, got: {stderr}"
        );
    }
    assert!(
        !stderr.contains("panic"),
        "stderr must not contain a panic trace, got: {stderr}"
    );
    assert!(
        !packed.exists(),
        "no container should be written on usage error"
    );

    let _ = fs::remove_dir_all(base);
}

/// Issue #161: table-driven stdin pack contract — core success round-trip and exit-2
/// rejections for unsupported combinations (non-core profile, missing -o).
#[test]
fn stdin_pack_table_driven_core_success_and_rejections() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-stdin-table-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    // Success: streaming core path round-trips minimal stdin through pack/export.
    let success_input = b"x";
    let packed = base.join("core-success.qzt");
    let packed_str = packed.to_str().expect("output path is utf-8");
    let pack_output = run_stdin_pack(&["-o", packed_str, "--profile", "core"], success_input);
    assert!(
        pack_output.status.success(),
        "core stdin pack must succeed, stderr: {}",
        String::from_utf8_lossy(&pack_output.stderr)
    );
    assert!(packed.exists(), "packed container must exist");

    let exported = output_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("export")
            .arg(&packed),
    );
    assert_eq!(
        exported, success_input,
        "export must restore original stdin bytes"
    );

    let rejections = [
        StdinPackRejectionCase {
            name: "non-core archive profile",
            extra_pack_args: &["--profile", "archive"],
            stderr_needles: &["stdin", "archive", "--profile core"],
            with_output_path: true,
        },
        StdinPackRejectionCase {
            name: "missing -o",
            extra_pack_args: &[],
            stderr_needles: &["missing -o"],
            with_output_path: false,
        },
    ];

    for case in rejections {
        let reject_packed = base.join(format!("reject-{}.qzt", case.name.replace(' ', "-")));
        let reject_packed_str = reject_packed.to_str().expect("output path is utf-8");

        let mut pack_args = Vec::new();
        if case.with_output_path {
            pack_args.extend_from_slice(&["-o", reject_packed_str]);
        }
        pack_args.extend_from_slice(case.extra_pack_args);

        let output = run_stdin_pack(&pack_args, b"alpha\nbeta\n");

        assert_eq!(
            output.status.code(),
            Some(2),
            "{}: stdin pack must exit 2",
            case.name
        );
        assert!(
            output.stdout.is_empty(),
            "{}: stdout must be empty on usage error, got: {:?}",
            case.name,
            String::from_utf8_lossy(&output.stdout)
        );

        let stderr = String::from_utf8_lossy(&output.stderr);
        for needle in case.stderr_needles {
            assert!(
                stderr.contains(needle),
                "{}: stderr must contain {:?}, got: {stderr}",
                case.name,
                needle
            );
        }
        assert!(
            !stderr.contains("panic"),
            "{}: stderr must not contain a panic trace, got: {stderr}",
            case.name
        );

        if case.with_output_path {
            assert!(
                !reject_packed.exists(),
                "{}: no container should be written on usage error",
                case.name
            );
        }
    }

    let _ = fs::remove_dir_all(base);
}

/// Issue #115: `qzt pack -` with `--profile memory` exits 2 and names the unsupported profile.
#[test]
fn stdin_pack_memory_profile_conflict_exits_2_with_clear_stderr() {
    let base = std::env::temp_dir().join(format!(
        "qzt-phase9-stdin-memory-conflict-{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&base);
    let packed = base.join("never.qzt");

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("pack")
        .arg("-")
        .arg("-o")
        .arg(&packed)
        .arg("--profile")
        .arg("memory")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("qzt pack should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdin + --profile memory must exit 2"
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
        stderr.contains("memory"),
        "stderr must name the unsupported profile, got: {stderr}"
    );
    assert!(
        stderr.contains("--profile core"),
        "stderr must point to --profile core, got: {stderr}"
    );
    assert!(
        stderr.contains("pack_bytes_with_memory_profile"),
        "stderr must mention the writer API path, got: {stderr}"
    );
    assert!(
        !packed.exists(),
        "no container should be written on usage error"
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

/// Regression guard for `qzt verify --format json` success contract: exit 0, single JSON
/// object on stdout (`ok`, `level`, `checked_chunks`, `decoded_bytes`), silent stderr.
#[test]
fn verify_json_success_stdout_only() {
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

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("verify")
        .arg(&packed)
        .arg("--format")
        .arg("json")
        .output()
        .expect("command should run");

    assert_eq!(output.status.code(), Some(0), "valid container must exit 0");

    assert!(
        output.stderr.is_empty(),
        "stderr must be empty on success, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json = String::from_utf8(output.stdout).expect("json output should be utf-8");
    assert!(
        !json.trim().contains('\n'),
        "stdout must be a single JSON object line, got: {json}"
    );

    let value: serde_json::Value =
        serde_json::from_str(json.trim()).expect("success output must be valid JSON");
    assert!(value.is_object(), "stdout must be a JSON object: {json}");

    assert_eq!(
        value.get("ok").and_then(serde_json::Value::as_bool),
        Some(true),
        "ok must be true: {json}"
    );
    assert_eq!(
        value.get("level").and_then(serde_json::Value::as_str),
        Some("normal"),
        "level must be normal: {json}"
    );
    let checked_chunks = value
        .get("checked_chunks")
        .and_then(serde_json::Value::as_u64)
        .expect("checked_chunks must be present");
    assert!(checked_chunks >= 1, "checked_chunks must be >= 1: {json}");
    assert!(
        value
            .get("decoded_bytes")
            .and_then(serde_json::Value::as_u64)
            .is_some(),
        "decoded_bytes must be a numeric value: {json}"
    );

    let _ = fs::remove_dir_all(base);
}

/// `qzt verify --format json` must always emit `decoded_bytes` fixed by level and
/// `checked_chunks` consistent across levels for the same container.
#[test]
fn verify_json_decoded_bytes_are_fixed_by_level() {
    let base = std::env::temp_dir().join(format!("qzt-phase9-vjson-levels-{}", std::process::id()));
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

    let original_size = content.len() as u64;
    let mut checked_chunks: Option<u64> = None;

    for (level_flag, level_name, expected_decoded_bytes) in [
        ("--quick", "quick", 0_u64),
        ("--normal", "normal", 0_u64),
        ("--deep", "deep", original_size),
    ] {
        let json_bytes = output_success(
            Command::new(env!("CARGO_BIN_EXE_qzt"))
                .arg("verify")
                .arg(&packed)
                .arg(level_flag)
                .arg("--format")
                .arg("json"),
        );
        let json = String::from_utf8(json_bytes).expect("json output should be utf-8");
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("success output must be valid JSON");

        assert_eq!(
            value.get("ok").and_then(serde_json::Value::as_bool),
            Some(true),
            "ok must be true for {level_name}: {json}"
        );
        assert_eq!(
            value.get("level").and_then(serde_json::Value::as_str),
            Some(level_name),
            "level must be {level_name}: {json}"
        );
        assert_eq!(
            value
                .get("decoded_bytes")
                .and_then(serde_json::Value::as_u64),
            Some(expected_decoded_bytes),
            "decoded_bytes must be {expected_decoded_bytes} for {level_name}: {json}"
        );

        let chunks = value
            .get("checked_chunks")
            .and_then(serde_json::Value::as_u64)
            .expect("checked_chunks must be present");
        assert!(
            chunks >= 1,
            "checked_chunks must be >= 1 for {level_name}: {json}"
        );

        match checked_chunks {
            None => checked_chunks = Some(chunks),
            Some(expected) => assert_eq!(
                chunks, expected,
                "checked_chunks must match across levels: {json}"
            ),
        }
    }

    let _ = fs::remove_dir_all(base);
}

/// Regression guard for the frozen `qzt verify --deep --format json` failure contract:
/// valid JSON on stdout, `ok:false`, `level:"deep"`, non-empty `error` string, silent stderr.
#[test]
fn verify_json_error_contract_is_stable() {
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
    let value: serde_json::Value =
        serde_json::from_str(&json).expect("failure output must be valid JSON");

    assert_eq!(
        value.get("ok").and_then(serde_json::Value::as_bool),
        Some(false),
        "ok must be false: {json}"
    );
    assert_eq!(
        value.get("level").and_then(serde_json::Value::as_str),
        Some("deep"),
        "level must be deep: {json}"
    );
    let error = value
        .get("error")
        .and_then(serde_json::Value::as_str)
        .expect("error must be a string");
    assert!(!error.is_empty(), "error must be non-empty: {json}");

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
