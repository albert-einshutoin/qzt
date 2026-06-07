use std::fs;
use std::process::Command;

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
    assert!(info.contains("Original size: 17"));
    assert!(info.contains("Chunks:"));
    assert!(info.contains("Lines: 3"));

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
