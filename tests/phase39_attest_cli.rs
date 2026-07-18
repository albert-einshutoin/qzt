use std::fmt::Write as _;
use std::fs;
use std::process::{Command, Output, Stdio};

#[cfg(feature = "internal-testing")]
use qzt::{Checksum, fixed::FooterTrailer, format::FOOTER_TRAILER_LEN, schema::FooterPayload};
use qzt::{QztFileReader, WriterOptions, pack_bytes_with_container_id};

const SOURCE: &[u8] = b"alpha\nbeta\ngamma\n";

fn fixture(label: &str) -> (std::path::PathBuf, Vec<u8>) {
    let base = std::env::temp_dir().join(format!("qzt-phase39-{label}-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let path = base.join("evidence.qzt");
    let bytes = pack_bytes_with_container_id(SOURCE, [0x39; 16], WriterOptions::default())
        .expect("fixture should pack");
    fs::write(&path, &bytes).expect("fixture should be writable");
    (path, bytes)
}

fn run_attest(path: &std::path::Path, extra_args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("attest")
        .arg(path)
        .args(extra_args)
        .output()
        .expect("qzt attest should run")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        },
    )
}

#[test]
fn attest_output_is_deterministic_and_canonical() {
    let (path, _) = fixture("canonical");

    let first = run_attest(&path, &[]);
    let second = run_attest(&path, &[]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(first.stdout, second.stdout);
    assert!(first.stderr.is_empty());

    let reader = QztFileReader::open_path(&path).expect("fixture should open");
    let info = reader.info();
    let details = reader.skeleton_details();
    let metadata = &details.metadata;
    let footer = &details.footer_payload;
    let container_checksum = footer
        .container_checksum
        .as_ref()
        .expect("writer fixtures include a container checksum");
    let expected = format!(
        concat!(
            "{{\"chunk_count\":{chunk_count},",
            "\"container_checksum\":{{\"algorithm\":\"{container_algorithm}\",\"value\":\"{container_value}\"}},",
            "\"container_id\":\"{container_id}\",",
            "\"final_file_size\":{final_file_size},",
            "\"format\":\"qzt-0.1\",",
            "\"line_count\":{line_count},",
            "\"original_checksum\":{{\"algorithm\":\"{original_algorithm}\",\"value\":\"{original_value}\"}},",
            "\"original_size\":{original_size},",
            "\"verify\":{{\"checked_chunks\":{checked_chunks},\"decoded_bytes\":{decoded_bytes},\"level\":\"deep\"}}}}\n"
        ),
        chunk_count = info.chunk_count,
        container_algorithm = container_checksum.algorithm,
        container_value = hex(&container_checksum.value),
        container_id = hex(&info.container_id),
        final_file_size = footer.final_file_size,
        line_count = info.line_count,
        original_algorithm = metadata.original_checksum.algorithm,
        original_value = hex(&metadata.original_checksum.value),
        original_size = info.original_size,
        checked_chunks = info.chunk_count,
        decoded_bytes = info.original_size,
    );
    assert_eq!(String::from_utf8(first.stdout).unwrap(), expected);
}

#[test]
fn attest_refuses_corrupt_container_without_stdout() {
    let (path, mut bytes) = fixture("corrupt");
    let reader = QztFileReader::open_path(&path).expect("fixture should open");
    let chunk_offset = usize::try_from(reader.skeleton_details().chunk_entries[0].physical_offset)
        .expect("fixture offset should fit usize");
    bytes[chunk_offset] ^= 0x01;
    fs::write(&path, bytes).expect("corrupt fixture should be writable");

    let output = run_attest(&path, &[]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "failed attest must not emit a claim"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("qzt attest:"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(feature = "internal-testing")]
#[test]
fn attest_emits_null_for_legacy_container_without_container_checksum() {
    let (path, bytes) = fixture("no-container-checksum");
    fs::write(&path, without_container_checksum(&bytes))
        .expect("legacy fixture should be writable");

    let output = run_attest(&path, &[]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["container_checksum"], serde_json::Value::Null);
    assert_eq!(value["verify"]["level"], "deep");
}

#[cfg(feature = "internal-testing")]
fn without_container_checksum(bytes: &[u8]) -> Vec<u8> {
    let details = qzt::open_skeleton_details(bytes).expect("fixture should open");
    let footer_offset = details.footer_payload_offset;
    let mut final_file_size = 0_u64;

    for _ in 0..8 {
        let footer = FooterPayload {
            container_id: details.footer_payload.container_id,
            index_root: details.footer_payload.index_root.clone(),
            metadata: details.footer_payload.metadata.clone(),
            final_file_size,
            footer_flags: details.footer_payload.footer_flags,
            container_checksum: None,
        };
        let footer_bytes = footer.encode().expect("footer should encode");
        let next_file_size = footer_offset
            + u64::try_from(footer_bytes.len()).expect("footer length should fit u64")
            + u64::try_from(FOOTER_TRAILER_LEN).expect("trailer length should fit u64");
        if next_file_size == final_file_size {
            let trailer = FooterTrailer {
                footer_payload_offset: footer_offset,
                footer_payload_size: u64::try_from(footer_bytes.len())
                    .expect("footer length should fit u64"),
                footer_payload_checksum_blake3: Checksum::blake3(&footer_bytes).value,
            };
            let prefix_end = usize::try_from(footer_offset).expect("fixture offset should fit");
            let mut output = bytes[..prefix_end].to_vec();
            output.extend_from_slice(&footer_bytes);
            output.extend_from_slice(&trailer.encode());
            return output;
        }
        final_file_size = next_file_size;
    }

    panic!("footer size should converge")
}

#[test]
fn attest_records_requested_verify_level() {
    let (path, _) = fixture("levels");

    for (level, decoded_bytes) in [("quick", 0), ("normal", 0), ("deep", SOURCE.len())] {
        let output = run_attest(&path, &["--level", level]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("attestation should be JSON");
        assert_eq!(value["verify"]["level"], level);
        assert_eq!(value["verify"]["decoded_bytes"], decoded_bytes as u64);
    }
}

#[test]
fn attest_accepts_options_before_file_as_documented() {
    let (path, _) = fixture("option-order");
    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(["attest", "--level", "quick"])
        .arg(path)
        .output()
        .expect("qzt attest should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["verify"]["level"], "quick");
}

#[test]
fn attest_defaults_to_deep_verify() {
    let (path, _) = fixture("default");
    let output = run_attest(&path, &[]);
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["verify"]["level"], "deep");
    assert_eq!(value["verify"]["decoded_bytes"], SOURCE.len() as u64);
}

#[test]
fn attest_rejects_invalid_level_as_usage_error() {
    let (path, _) = fixture("invalid-level");
    let output = run_attest(&path, &["--level", "shallow"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid --level value"));
}

#[test]
fn attest_help_documents_the_stable_output_contract() {
    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(["attest", "--help"])
        .output()
        .expect("qzt attest --help should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Usage: qzt attest [OPTIONS] <FILE>"));
    assert!(stdout.contains("--level <LEVEL>"));
    assert!(stdout.contains("default: deep"));
    assert!(stdout.contains("canonical JSON"));
}

#[test]
fn attest_reports_stdout_delivery_failure() {
    let (path, _) = fixture("closed-stdout");
    let mut child = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("attest")
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("qzt attest should start");

    // Close the pipe before the child writes. A canonical attestation is only
    // successful when the caller can actually receive the complete bytes.
    drop(child.stdout.take());
    let output = child.wait_with_output().expect("qzt attest should finish");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("failed to write stdout"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
