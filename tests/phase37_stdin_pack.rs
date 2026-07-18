/// Integration tests for `qzt pack -` (stdin input, issue #37).
///
/// Verifies that:
/// - A non-empty stdin stream packs and round-trips correctly via export.
/// - An empty stdin stream produces a valid container (empty round-trip).
/// - Using stdin (`-`) with a non-streaming profile exits 2 with a clear message.
/// - File input behaviour is completely unchanged.
use std::fs;
use std::io::Write as _;
use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_qzt"))
}

fn export_to_bytes(packed: &std::path::Path) -> Vec<u8> {
    let out = bin()
        .args(["export", packed.to_str().unwrap()])
        .output()
        .expect("export should run");
    assert!(
        out.status.success(),
        "export failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

fn verify_deep(packed: &std::path::Path) {
    let out = bin()
        .args(["verify", packed.to_str().unwrap(), "--deep"])
        .output()
        .expect("deep verify should run");
    assert!(
        out.status.success(),
        "deep verify failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---------------------------------------------------------------------------
// pack_reads_stdin_and_roundtrips
// ---------------------------------------------------------------------------

/// `qzt pack - -o out.qzt` with non-empty stdin packs and round-trips correctly.
#[test]
fn pack_reads_stdin_and_roundtrips() {
    let dir = std::env::temp_dir().join(format!("qzt-37-stdin-rt-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let out = dir.join("stdin.qzt");

    let mut child = bin()
        .args(["pack", "-", "-o", out.to_str().unwrap()])
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"line1\nline2\n")
        .unwrap();
    // Close the write end of the pipe so `qzt pack` receives EOF and can exit.
    drop(child.stdin.take());
    let status = child.wait().unwrap();
    assert!(
        status.success(),
        "pack from stdin must succeed (exit 0), got: {status:?}"
    );
    assert!(out.exists(), "output container must exist");

    // Round-trip: export must restore the original bytes.
    let restored = export_to_bytes(&out);
    assert_eq!(
        restored, b"line1\nline2\n",
        "round-trip must preserve original bytes"
    );

    let _ = fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------
// pack_stdin_empty_input_roundtrips
// ---------------------------------------------------------------------------

/// `qzt pack -` with zero bytes on stdin produces a valid (empty) container.
///
/// Empty containers are already supported by the format (see `tests/vectors/valid_empty`).
#[test]
fn pack_stdin_empty_input_roundtrips() {
    let dir = std::env::temp_dir().join(format!("qzt-37-stdin-empty-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let out = dir.join("stdin_empty.qzt");

    let mut child = bin()
        .args(["pack", "-", "-o", out.to_str().unwrap()])
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn");
    // Close stdin immediately (0 bytes).
    drop(child.stdin.take());
    let status = child.wait().unwrap();
    assert!(
        status.success(),
        "packing empty stdin must succeed, got: {status:?}"
    );
    assert!(out.exists(), "output container must exist for empty input");

    verify_deep(&out);

    // Exporting an empty container must yield 0 bytes.
    let restored = export_to_bytes(&out);
    assert!(
        restored.is_empty(),
        "empty round-trip must produce empty output, got {} byte(s)",
        restored.len()
    );

    let _ = fs::remove_dir_all(dir);
}

/// A one-byte stdin input exercises the smallest non-empty streaming chunk.
#[test]
fn pack_stdin_one_byte_deep_verifies_and_roundtrips() {
    let dir = std::env::temp_dir().join(format!("qzt-37-stdin-one-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let out = dir.join("stdin_one.qzt");

    let mut child = bin()
        .args(["pack", "-", "-o", out.to_str().unwrap()])
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"x")
        .expect("write one-byte stdin");
    drop(child.stdin.take());

    let status = child.wait().unwrap();
    assert!(
        status.success(),
        "packing one-byte stdin must succeed, got: {status:?}"
    );
    verify_deep(&out);
    assert_eq!(export_to_bytes(&out), b"x");

    let _ = fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------
// pack_stdin_rejects_non_streaming_profile
// ---------------------------------------------------------------------------

/// `qzt pack - --profile memory` (non-streaming profile) must exit 2 and emit
/// a message containing "stdin" to stderr.
#[test]
fn pack_stdin_rejects_non_streaming_profile() {
    let dir = std::env::temp_dir().join(format!("qzt-37-stdin-badprof-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let out = dir.join("never.qzt");

    let output = bin()
        .args([
            "pack",
            "-",
            "-o",
            out.to_str().unwrap(),
            "--profile",
            "memory",
        ])
        .stdin(Stdio::piped())
        .output()
        .expect("command should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "non-streaming profile + stdin must exit 2, got: {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("stdin"),
        "stderr must mention 'stdin', got: {stderr}"
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
        stderr.contains("WriterBuilder"),
        "stderr must mention the writer API path, got: {stderr}"
    );
    // The container must NOT have been created.
    assert!(
        !out.exists(),
        "no container should be written on usage error"
    );

    let _ = fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------
// pack_stdin_rejects_dense_line_index
// ---------------------------------------------------------------------------

/// `qzt pack - --dense-line-index on` must exit 2 because dense line index
/// forces the in-memory path and cannot be used with stdin.
#[test]
fn pack_stdin_rejects_dense_line_index() {
    let dir = std::env::temp_dir().join(format!("qzt-37-stdin-dense-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let out = dir.join("never.qzt");

    let output = bin()
        .args([
            "pack",
            "-",
            "-o",
            out.to_str().unwrap(),
            "--dense-line-index",
            "on",
        ])
        .stdin(Stdio::piped())
        .output()
        .expect("command should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "--dense-line-index on + stdin must exit 2"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("stdin"),
        "stderr must mention 'stdin', got: {stderr}"
    );
    assert!(
        stderr.contains("--dense-line-index on"),
        "stderr must name --dense-line-index on, got: {stderr}"
    );
    assert!(
        stderr.contains("Dense Line Index"),
        "stderr must name the Dense Line Index restriction, got: {stderr}"
    );
    assert!(
        stderr.contains("--profile core"),
        "stderr must point to --profile core, got: {stderr}"
    );
    assert!(
        !out.exists(),
        "no container should be written on usage error"
    );

    let _ = fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------
// pack_file_input_unchanged
// ---------------------------------------------------------------------------

/// File input behaviour is completely unchanged after the stdin feature.
#[test]
fn pack_file_input_unchanged() {
    let dir = std::env::temp_dir().join(format!("qzt-37-file-unch-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("input.txt");
    let out = dir.join("output.qzt");
    fs::write(&input, b"alpha\nbeta\ngamma\n").unwrap();

    let status = bin()
        .args(["pack", input.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .status()
        .expect("pack should run");
    assert!(status.success(), "file pack must succeed");

    let restored = export_to_bytes(&out);
    assert_eq!(
        restored, b"alpha\nbeta\ngamma\n",
        "file pack round-trip must be unchanged"
    );

    let _ = fs::remove_dir_all(dir);
}
