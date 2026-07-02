/// CLI contract tests for stdin pack rejection on non-streaming profiles (issue #56).
///
/// `qzt pack -` is only supported on the streaming `--profile core` path without
/// `--dense-line-index on`. Other combinations must exit 2 with a clear stderr
/// message so large stdin inputs are never buffered silently.
use std::fs;
use std::process::{Command, Output, Stdio};

const STDIN_STREAMING_MSG: &str = "stdin is only supported on the streaming pack path";

struct StdinPackRejectionCase {
    name: &'static str,
    extra_args: &'static [&'static str],
}

fn run_pack_stdin(extra_args: &[&str], out: &str) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_qzt"));
    cmd.args(["pack", "-", "-o", out]);
    for arg in extra_args {
        cmd.arg(arg);
    }
    cmd.stdin(Stdio::piped())
        .output()
        .expect("command should run")
}

/// Non-streaming stdin pack paths exit 2 and explain the streaming-only contract.
#[test]
fn stdin_pack_rejects_non_streaming_paths() {
    let base = std::env::temp_dir().join(format!("qzt-cli-stdin-pack-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    let cases = [
        StdinPackRejectionCase {
            name: "memory profile",
            extra_args: &["--profile", "memory"],
        },
        StdinPackRejectionCase {
            name: "dense line index on",
            extra_args: &["--dense-line-index", "on"],
        },
    ];

    for case in cases {
        let out = base.join(format!("never-{}.qzt", case.name.replace(' ', "-")));
        let out_str = out.to_str().expect("output path is utf-8");

        let output = run_pack_stdin(case.extra_args, out_str);

        assert_eq!(
            output.status.code(),
            Some(2),
            "{}: stdin pack must exit 2, got {:?}",
            case.name,
            output.status.code()
        );

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(STDIN_STREAMING_MSG),
            "{}: stderr must contain {:?}, got: {stderr}",
            case.name,
            STDIN_STREAMING_MSG
        );

        assert!(
            !out.exists(),
            "{}: no container should be written on usage error",
            case.name
        );
    }

    let _ = fs::remove_dir_all(base);
}
