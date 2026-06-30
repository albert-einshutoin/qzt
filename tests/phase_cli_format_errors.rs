/// Table-driven CLI contract tests for unknown `--format` values (issue #57).
///
/// Commands that accept `--format text|json` must reject unknown values with
/// exit code 2 and a stderr message naming the bad value and accepted formats.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use qzt::{Checksum, DocumentEntry, DocumentIndex, WriterOptions, pack_bytes_with_document_index};

struct UnknownFormatCase {
    command: &'static str,
    /// Extra arguments between the container path and `--format`.
    middle_args: &'static [&'static str],
    unknown_format: &'static str,
    /// When true, use a Document Index container required by `qzt docs`.
    needs_document_index: bool,
}

fn run(command: &str, qzt: &str, middle: &[&str], format: &str) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_qzt"));
    cmd.arg(command).arg(qzt);
    for arg in middle {
        cmd.arg(arg);
    }
    cmd.arg("--format").arg(format);
    cmd.output().expect("command should run")
}

fn pack_plain(base: &Path) -> PathBuf {
    let input_path = base.join("input.txt");
    let packed_path = base.join("plain.qzt");
    fs::write(&input_path, b"hello\nworld\n").expect("input write");

    let out = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args([
            "pack",
            input_path.to_str().unwrap(),
            "-o",
            packed_path.to_str().unwrap(),
        ])
        .output()
        .expect("pack should run");
    assert!(
        out.status.success(),
        "pack failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    packed_path
}

fn pack_with_document_index(base: &Path) -> PathBuf {
    const INPUT: &[u8] = b"hello\n";
    let doc = DocumentEntry::new("doc-one", 0, 6, 0, 1, 0, 1, Checksum::blake3(&INPUT[0..6]));
    let document_index = DocumentIndex {
        container_id: [0xab; 16],
        documents: vec![doc],
    };
    let bytes = pack_bytes_with_document_index(
        INPUT,
        [0xab; 16],
        WriterOptions::default(),
        &document_index,
    )
    .expect("document index pack should work");

    let packed_path = base.join("docs.qzt");
    fs::write(&packed_path, bytes).expect("packed container write");
    packed_path
}

/// Unknown `--format` values exit 2 with stderr naming the bad value and accepted formats.
#[test]
fn unknown_format_exits_2_with_message() {
    let base = std::env::temp_dir().join(format!("qzt-cli-format-errors-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    let plain_qzt = pack_plain(&base);
    let docs_qzt = pack_with_document_index(&base);
    let plain = plain_qzt.to_str().expect("plain path is utf-8");
    let docs = docs_qzt.to_str().expect("docs path is utf-8");

    let cases = [
        UnknownFormatCase {
            command: "info",
            middle_args: &[],
            unknown_format: "yaml",
            needs_document_index: false,
        },
        UnknownFormatCase {
            command: "verify",
            middle_args: &[],
            unknown_format: "yaml",
            needs_document_index: false,
        },
        UnknownFormatCase {
            command: "search",
            middle_args: &["hello"],
            unknown_format: "csv",
            needs_document_index: false,
        },
        UnknownFormatCase {
            command: "docs",
            middle_args: &[],
            unknown_format: "yaml",
            needs_document_index: true,
        },
    ];

    for case in cases {
        let qzt = if case.needs_document_index {
            docs
        } else {
            plain
        };
        let out = run(case.command, qzt, case.middle_args, case.unknown_format);

        assert_eq!(
            out.status.code(),
            Some(2),
            "{} --format {} must exit 2",
            case.command,
            case.unknown_format
        );

        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains(case.unknown_format),
            "{} stderr must name unsupported format {:?}: {}",
            case.command,
            case.unknown_format,
            stderr
        );
        assert!(
            stderr.contains("text") && stderr.contains("json"),
            "{} stderr must list accepted formats text and json: {}",
            case.command,
            stderr
        );
    }

    let _ = fs::remove_dir_all(base);
}
