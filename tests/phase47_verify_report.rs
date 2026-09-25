use qzt::{
    DocumentIndex, DocumentSpan, QztFileReader, QztReader, VerifyLevel, WriterBuilder,
    WriterOptions, pack_bytes_with_container_id,
};
use qzt::{IndexVerificationStatus as IndexStatus, PrefixChecksumStatus as PrefixStatus};
use std::fs;
use std::process::Command;
mod support;

fn valid_c1() -> Vec<u8> {
    include_str!("vectors/valid_c1.qzt.hex")
        .trim()
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn verify_json_distinguishes_structural_checksum_and_decode_work() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("valid_c1.qzt");
    fs::write(&path, valid_c1()).unwrap();

    for (level, compressed, decoded, bytes, original, prefix) in [
        ("quick", 0, 0, 0, false, "present_unchecked"),
        ("normal", 1, 0, 0, false, "verified"),
        ("deep", 1, 1, 11, true, "verified"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
            .args([
                "verify",
                path.to_str().unwrap(),
                &format!("--{level}"),
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["checked_chunks"], 1);
        assert_eq!(report["compressed_checksum_chunks"], compressed);
        assert_eq!(report["decoded_chunks"], decoded);
        assert_eq!(report["decoded_bytes"], bytes);
        assert_eq!(report["original_checksum_verified"], original);
        assert_eq!(report["container_checksum_status"], prefix);
    }
}

#[test]
fn text_and_json_name_the_same_deep_work() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("valid_c1.qzt");
    fs::write(&path, valid_c1()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(["verify", path.to_str().unwrap(), "--deep"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("Verify: Deep ok\nChecked chunks: 1\nDecoded bytes: 11\n"));
    for line in [
        "Compressed checksum chunks: 1",
        "Decoded chunks: 1",
        "Original checksum verified: true",
        "Container prefix checksum: verified",
        "Dense Line Index: absent",
        "Document Index: absent",
    ] {
        assert!(
            text.lines().any(|actual| actual == line),
            "missing {line}: {text}"
        );
    }
}

#[test]
fn both_readers_report_each_pass_without_inheriting_prior_work() {
    let input = b"a\nb\nc\nd\n";
    let bytes = WriterBuilder::new()
        .container_id([0x47; 16])
        .options(support::writer_options(4, 4))
        .dense_line_index(true)
        .document_spans(vec![
            DocumentSpan::new("first", 0, 4),
            DocumentSpan::new("second", 4, 4),
        ])
        .pack(input)
        .unwrap();
    let memory = QztReader::open(&bytes).unwrap();
    let file = QztFileReader::open_read_at(&bytes[..], bytes.len() as u64).unwrap();
    let count = memory.info().chunk_count;
    assert!(count > 1);
    for level in [VerifyLevel::Quick, VerifyLevel::Normal, VerifyLevel::Deep] {
        let report = memory.verify(level).unwrap();
        assert_eq!(report, file.verify(level).unwrap());
        assert_eq!(report.checked_chunks, count);
        assert_eq!(
            report.compressed_checksum_chunks,
            if level == VerifyLevel::Quick {
                0
            } else {
                count
            }
        );
        assert_eq!(
            report.decoded_chunks,
            if level == VerifyLevel::Deep { count } else { 0 }
        );
        assert_eq!(
            report.decoded_bytes,
            if level == VerifyLevel::Deep {
                input.len() as u64
            } else {
                0
            }
        );
        assert_eq!(
            report.original_checksum_verified,
            level == VerifyLevel::Deep
        );
        assert_eq!(
            report.container_checksum_status,
            if level == VerifyLevel::Quick {
                PrefixStatus::PresentUnchecked
            } else {
                PrefixStatus::Verified
            }
        );
        let index_status = if level == VerifyLevel::Deep {
            IndexStatus::SourceChecked
        } else {
            IndexStatus::StoredBlockVerified
        };
        assert_eq!(report.dense_line_index_status, index_status);
        assert_eq!(report.document_index_status, index_status);
    }
    let quick_after_deep = memory.verify(VerifyLevel::Quick).unwrap();
    assert_eq!(quick_after_deep.compressed_checksum_chunks, 0);
    assert_eq!(quick_after_deep.decoded_chunks, 0);
    assert!(!quick_after_deep.original_checksum_verified);
}

#[test]
fn empty_present_indexes_and_empty_original_checksum_are_distinct() {
    let container_id = [0x48; 16];
    let bytes = WriterBuilder::new()
        .container_id(container_id)
        .dense_line_index(true)
        .document_index(DocumentIndex {
            container_id,
            documents: vec![],
        })
        .pack(b"")
        .unwrap();
    let memory = QztReader::open(&bytes).unwrap();
    let file = QztFileReader::open_read_at(&bytes[..], bytes.len() as u64).unwrap();
    for reader_report in [
        memory.verify(VerifyLevel::Deep).unwrap(),
        file.verify(VerifyLevel::Deep).unwrap(),
    ] {
        assert_eq!(reader_report.checked_chunks, 0);
        assert_eq!(reader_report.decoded_chunks, 0);
        assert_eq!(reader_report.decoded_bytes, 0);
        assert!(reader_report.original_checksum_verified);
        assert_eq!(
            reader_report.dense_line_index_status,
            IndexStatus::SourceChecked
        );
        assert_eq!(
            reader_report.document_index_status,
            IndexStatus::SourceChecked
        );
    }
    let quick = file.verify(VerifyLevel::Quick).unwrap();
    assert_eq!(
        quick.dense_line_index_status,
        IndexStatus::StoredBlockVerified
    );
    assert_eq!(
        quick.document_index_status,
        IndexStatus::StoredBlockVerified
    );
}

#[test]
fn quick_reads_no_chunk_payload_and_reports_no_payload_verification() {
    let bytes =
        pack_bytes_with_container_id(b"payload\n", [0x49; 16], WriterOptions::default()).unwrap();
    let source = support::CountingReadAt::new(bytes);
    let file = QztFileReader::open_read_at(source.clone(), source.bytes.len() as u64).unwrap();
    source.reads.lock().unwrap().clear();
    let report = file.verify(VerifyLevel::Quick).unwrap();
    assert!(source.reads.lock().unwrap().is_empty());
    assert_eq!(report.checked_chunks, 1);
    assert_eq!(report.compressed_checksum_chunks, 0);
    assert_eq!(report.decoded_chunks, 0);
    assert!(!report.original_checksum_verified);
}

#[test]
fn canonical_attestation_v1_matches_fixed_bytes_and_legacy_is_distinct() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("valid_c1.qzt");
    fs::write(&path, valid_c1()).unwrap();
    let first = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(["attest", path.to_str().unwrap()])
        .output()
        .unwrap();
    let second = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(["attest", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(
        first.stdout,
        include_bytes!("fixtures/attestation_v1_valid_c1.json")
    );
    assert_eq!(first.stdout.last(), Some(&b'\n'));
    assert_ne!(first.stdout[first.stdout.len() - 2], b'\n');
    let legacy = include_bytes!("fixtures/attestation_legacy_valid_c1.json");
    assert!(legacy.starts_with(b"{\"chunk_count\":"));
    assert_ne!(legacy.as_slice(), first.stdout);
}
