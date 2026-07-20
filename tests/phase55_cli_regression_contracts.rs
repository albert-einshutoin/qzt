use std::fs;
use std::io::ErrorKind;
use std::process::Command;

use qzt::QztError;
mod support;
use support::{assert_success, output_success};

#[test]
fn line_stdout_is_byte_identical_to_the_same_single_line_range() {
    let base = crate::support::secure_temp_root()
        .join(format!("qzt-line-range-equality-{}", std::process::id()));
    fs::create_dir_all(&base).expect("fixture directory should be created");
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    let lines: [&[u8]; 3] = [b"first line\n", "中央の行\r\n".as_bytes(), b"final line"];
    fs::write(&input, lines.concat()).expect("fixture input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );

    for (line_number, expected) in (1_u64..).zip(lines) {
        let line_stdout = output_success(
            Command::new(env!("CARGO_BIN_EXE_qzt"))
                .arg("line")
                .arg(&packed)
                .arg(line_number.to_string()),
        );
        let range_stdout = output_success(
            Command::new(env!("CARGO_BIN_EXE_qzt"))
                .arg("range")
                .arg(&packed)
                .arg("--lines")
                .arg(format!("{line_number}:{line_number}")),
        );

        assert_eq!(line_stdout, expected, "line {line_number} changed bytes");
        assert_eq!(
            line_stdout, range_stdout,
            "line {line_number} must equal its single-line range"
        );
    }
}

fn assert_human_readable_display(error: QztError, raw_variant: &str, expected: &[&str]) {
    let display = error.to_string();
    let debug = format!("{error:?}");

    assert_ne!(display, debug, "{raw_variant} Display delegated to Debug");
    assert!(
        !display.contains(raw_variant),
        "{raw_variant} leaked into user-facing text: {display}"
    );
    for fragment in expected {
        assert!(
            display.contains(fragment),
            "{raw_variant} Display should contain {fragment:?}: {display}"
        );
    }
}

#[test]
fn qzt_error_display_keeps_representative_variants_human_readable() {
    for (error, raw_variant, expected) in [
        (
            QztError::InvalidMagic,
            "InvalidMagic",
            &["invalid magic", "QZT container"][..],
        ),
        (
            QztError::InvalidHeader,
            "InvalidHeader",
            &["header", "malformed"][..],
        ),
        (
            QztError::ContainerCorrupt,
            "ContainerCorrupt",
            &["container", "corrupt"][..],
        ),
        (
            QztError::UnsupportedVersion,
            "UnsupportedVersion",
            &["unsupported", "format version"][..],
        ),
        (
            QztError::ResourceLimitExceeded,
            "ResourceLimitExceeded",
            &["resource limit"][..],
        ),
        (
            QztError::DocumentNotFound,
            "DocumentNotFound",
            &["document", "not found"][..],
        ),
    ] {
        assert_human_readable_display(error, raw_variant, expected);
    }

    let io_error = QztError::Io(ErrorKind::NotFound);
    assert_human_readable_display(io_error, "Io(", &["I/O error:", "not found"]);
    assert!(
        !io_error.to_string().contains("NotFound"),
        "ErrorKind's Debug variant must not leak into Display"
    );
}
