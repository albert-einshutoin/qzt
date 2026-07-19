use std::fs;
use std::process::Command;

/// Exact profile list line pinned by issue #71.
const PACK_PROFILES_LINE: &str = "Profiles: minimal, core, log, archive, memory";

#[test]
fn pack_help_lists_supported_profiles() {
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
        stdout.contains(PACK_PROFILES_LINE),
        "pack --help must list supported profiles exactly:\n{stdout}"
    );
}

#[test]
fn pack_help_states_technical_preview_positioning() {
    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(["pack", "--help"])
        .output()
        .expect("qzt pack --help should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("help output should be UTF-8");
    assert!(
        stdout.contains("v0.1 technical preview"),
        "pack --help must state v0.1 technical preview positioning"
    );
    assert!(
        stdout.contains("not production-ready"),
        "pack --help must not imply production readiness"
    );
}

#[test]
fn pack_rejects_invalid_profile_with_usage_error() {
    let base = std::env::temp_dir().join(format!("qzt-cli-help-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    fs::write(&input, b"hello\n").expect("fixture input should be writable");
    let output_path = base.join("out.qzt");

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("pack")
        .arg(&input)
        .args(["--profile", "bogus"])
        .arg("-o")
        .arg(&output_path)
        .output()
        .expect("qzt pack with invalid profile should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "invalid --profile must exit 2, got {:?}",
        output.status.code()
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid --profile value"),
        "invalid profile must keep existing usage-error message, got: {stderr}"
    );
    assert!(
        !output_path.exists(),
        "invalid profile must not create output file"
    );
}

#[test]
fn every_subcommand_exposes_command_specific_help() {
    for (command, usage) in [
        ("info", "Usage: qzt info <FILE> [--format text|json]"),
        ("export", "Usage: qzt export <FILE> [-o <OUTPUT>]"),
        ("range", "Usage: qzt range <FILE> --bytes A:B|--lines A:B"),
        ("line", "Usage: qzt line <FILE> <LINE> [--zero-based]"),
        ("docs", "Usage: qzt docs <FILE> [--format text|json]"),
        (
            "doc",
            "Usage: qzt doc <FILE> <DOC_ID> [-o <OUTPUT>] [--no-verify]",
        ),
        ("search", "Usage: qzt search <FILE> <QUERY> [OPTIONS]"),
        (
            "inspect-sidecar",
            "Usage: qzt inspect-sidecar <FILE.qzt> --sidecar <FILE.qzi> [--format text|json]",
        ),
        (
            "sidecar-rebuild",
            "Usage: qzt sidecar-rebuild <FILE> -o <OUTPUT.qzi> [OPTIONS]",
        ),
        (
            "verify",
            "Usage: qzt verify <FILE> [--quick|--normal|--deep] [--format text|json]",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
            .args([command, "--help"])
            .output()
            .expect("subcommand help should run");
        assert!(
            output.status.success(),
            "{command} --help must exit 0, got {:?}; stderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty(), "{command} help wrote to stderr");
        let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
        assert!(
            stdout.contains(usage),
            "{command} help must contain {usage:?}, got:\n{stdout}"
        );
    }
}

#[test]
fn pack_accepts_options_before_the_input_path() {
    let base = std::env::temp_dir().join(format!("qzt-cli-options-first-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let output_path = base.join("out.qzt");
    fs::write(&input, b"hello\n").expect("fixture input should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("pack")
        .args(["--profile", "core", "--output"])
        .arg(&output_path)
        .arg(&input)
        .output()
        .expect("qzt pack should run");

    assert!(
        output.status.success(),
        "options-first pack must succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_path.exists());
    fs::remove_dir_all(base).expect("fixture directory should be removable");
}
