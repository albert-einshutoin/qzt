use std::path::Path;
use std::process::Command;

#[test]
fn library_can_be_imported() {
    assert_eq!(qzt::version(), env!("CARGO_PKG_VERSION"));
    assert_eq!(qzt::format::MAGIC, *b"QZT\0TXT1");
    assert_eq!(qzt::format::VERSION, 1);
}

#[test]
fn cli_responds_to_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("--help")
        .output()
        .expect("qzt binary should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("help output should be UTF-8");
    assert!(stdout.contains("Usage: qzt <COMMAND>"));
}

#[test]
fn cli_help_command_matches_help_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("help")
        .output()
        .expect("qzt binary should run");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("help output should be UTF-8");
    assert!(stdout.contains("Usage: qzt <COMMAND>"));
}

#[test]
fn fixture_directories_exist() {
    for path in [
        "tests/fixtures/source",
        "tests/fixtures/valid",
        "tests/fixtures/corrupt",
    ] {
        assert!(Path::new(path).is_dir(), "{path} should exist");
    }
}
