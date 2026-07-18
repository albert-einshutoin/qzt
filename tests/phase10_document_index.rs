use qzt::error::QztError;
use qzt::reader::{QztReader, VerifyLevel};
use qzt::schema::{Checksum, DocumentEntry, DocumentIndex};
use qzt::skeleton::open_skeleton_details;
use qzt::writer::WriterBuilder;
mod support;
use support::{DocumentFixture, document, writer_options};

fn pack_document_fixture(
    input: &[u8],
    container_id: [u8; 16],
    options: qzt::WriterOptions,
    document_index: &DocumentIndex,
) -> qzt::Result<Vec<u8>> {
    WriterBuilder::new()
        .container_id(container_id)
        .options(options)
        .document_index(document_index.clone())
        .pack(input)
}

fn pack_memory_fixture(
    input: &[u8],
    container_id: [u8; 16],
    options: qzt::WriterOptions,
    document_index: &DocumentIndex,
) -> qzt::Result<Vec<u8>> {
    WriterBuilder::new()
        .container_id(container_id)
        .options(options)
        .profile("memory")
        .document_index(document_index.clone())
        .pack(input)
}

#[test]
fn document_index_ranges_are_verified_by_deep_verify() {
    let input = b"doc-one\n";
    let document_index = DocumentIndex {
        container_id: [0xb0; 16],
        documents: vec![document(&DocumentFixture {
            doc_id: "doc-one",
            input,
            logical_offset: input.len() as u64 + 1,
            byte_length: 1,
            first_line: 0,
            line_count: 1,
            chunk_start: 0,
            chunk_end: 1,
        })],
    };
    let container =
        pack_document_fixture(input, [0xb0; 16], writer_options(64, 64), &document_index)
            .expect("document-index container should pack structurally");
    let reader = QztReader::open(container).expect("container should open structurally");

    assert_eq!(
        reader.verify(VerifyLevel::Deep),
        Err(QztError::LogicalRangeOutOfBounds)
    );
}

#[test]
fn document_index_chunk_range_inconsistency_is_rejected_by_deep_verify() {
    let input = b"doc-one\ndoc-two\n";
    let document_index = DocumentIndex {
        container_id: [0xb1; 16],
        documents: vec![document(&DocumentFixture {
            doc_id: "doc-one",
            input,
            logical_offset: 0,
            byte_length: 8,
            first_line: 0,
            line_count: 1,
            chunk_start: 1,
            chunk_end: 1,
        })],
    };
    let container = pack_document_fixture(input, [0xb1; 16], writer_options(8, 8), &document_index)
        .expect("document-index container should pack structurally");
    let reader = QztReader::open(container).expect("container should open structurally");

    assert_eq!(
        reader.verify(VerifyLevel::Deep),
        Err(QztError::ChunkTableInvalid)
    );
}

#[test]
fn memory_profile_writes_document_index_flags_and_deep_verifies() {
    let input = b"doc-one\ndoc-two\n";
    let document_index = DocumentIndex {
        container_id: [0xb2; 16],
        documents: vec![
            document(&DocumentFixture {
                doc_id: "doc-one",
                input,
                logical_offset: 0,
                byte_length: 8,
                first_line: 0,
                line_count: 1,
                chunk_start: 0,
                chunk_end: 1,
            }),
            document(&DocumentFixture {
                doc_id: "doc-two",
                input,
                logical_offset: 8,
                byte_length: 8,
                first_line: 1,
                line_count: 1,
                chunk_start: 1,
                chunk_end: 2,
            }),
        ],
    };
    let container = pack_memory_fixture(input, [0xb2; 16], writer_options(8, 8), &document_index)
        .expect("memory profile should pack");
    let details = open_skeleton_details(&container).expect("memory profile should open");

    assert_eq!(details.metadata.profile, "memory");
    // Two lines is below the memory-profile Dense Line Index threshold (2048).
    assert!(!details.metadata.dense_line_index);
    assert!(details.metadata.document_index);
    assert!(details.dense_line_index.is_none());
    assert!(details.document_index.is_some());

    let reader = QztReader::open(container).expect("memory profile reader should open");
    assert!(reader.verify(VerifyLevel::Deep).is_ok());
}

// --- Review follow-up coverage: single-pass document verification + lookup ---

const TWO_LINES: &[u8] = b"aaaaaaaa\nbbbbbbbb\n"; // 18 bytes, two 9-byte lines

fn two_chunk_container(documents: Vec<DocumentEntry>) -> Vec<u8> {
    let document_index = DocumentIndex {
        container_id: [0xc0; 16],
        documents,
    };
    // target/max 9 -> two chunks [0,9) and [9,18)
    pack_document_fixture(TWO_LINES, [0xc0; 16], writer_options(9, 9), &document_index)
        .expect("document-index container should pack structurally")
}

#[test]
fn document_spanning_multiple_chunks_deep_verifies() {
    let whole = document(&DocumentFixture {
        doc_id: "whole",
        input: TWO_LINES,
        logical_offset: 0,
        byte_length: 18,
        first_line: 0,
        line_count: 2,
        chunk_start: 0,
        chunk_end: 2,
    });
    let reader = QztReader::open(two_chunk_container(vec![whole])).expect("opens");
    assert!(reader.verify(VerifyLevel::Deep).is_ok());
}

#[test]
fn out_of_order_documents_deep_verify() {
    let second = document(&DocumentFixture {
        doc_id: "doc-two",
        input: TWO_LINES,
        logical_offset: 9,
        byte_length: 9,
        first_line: 1,
        line_count: 1,
        chunk_start: 1,
        chunk_end: 2,
    });
    let first = document(&DocumentFixture {
        doc_id: "doc-one",
        input: TWO_LINES,
        logical_offset: 0,
        byte_length: 9,
        first_line: 0,
        line_count: 1,
        chunk_start: 0,
        chunk_end: 1,
    });
    // Listed later-range-first on purpose.
    let reader = QztReader::open(two_chunk_container(vec![second, first])).expect("opens");
    assert!(reader.verify(VerifyLevel::Deep).is_ok());
}

#[test]
fn empty_document_deep_verifies_without_decoded_bytes() {
    let empty = document(&DocumentFixture {
        doc_id: "empty",
        input: TWO_LINES,
        logical_offset: 9,
        byte_length: 0,
        first_line: 1,
        line_count: 0,
        chunk_start: 0,
        chunk_end: 0,
    });
    let reader = QztReader::open(two_chunk_container(vec![empty])).expect("opens");
    assert!(reader.verify(VerifyLevel::Deep).is_ok());
}

#[test]
fn document_checksum_mismatch_is_rejected_by_deep_verify() {
    let mut wrong = document(&DocumentFixture {
        doc_id: "whole",
        input: TWO_LINES,
        logical_offset: 0,
        byte_length: 18,
        first_line: 0,
        line_count: 2,
        chunk_start: 0,
        chunk_end: 2,
    });
    wrong.checksum = Checksum::blake3(b"not the document bytes");
    let reader = QztReader::open(two_chunk_container(vec![wrong])).expect("opens");
    assert_eq!(
        reader.verify(VerifyLevel::Deep),
        Err(QztError::ContainerCorrupt)
    );
}

#[test]
fn read_document_resolves_by_id_and_reports_missing() {
    let first = document(&DocumentFixture {
        doc_id: "doc-one",
        input: TWO_LINES,
        logical_offset: 0,
        byte_length: 9,
        first_line: 0,
        line_count: 1,
        chunk_start: 0,
        chunk_end: 1,
    });
    let second = document(&DocumentFixture {
        doc_id: "doc-two",
        input: TWO_LINES,
        logical_offset: 9,
        byte_length: 9,
        first_line: 1,
        line_count: 1,
        chunk_start: 1,
        chunk_end: 2,
    });
    let reader = QztReader::open(two_chunk_container(vec![first, second])).expect("opens");

    assert_eq!(
        reader.read_document("doc-one").expect("doc-one"),
        b"aaaaaaaa\n"
    );
    assert_eq!(
        reader.read_document("doc-two").expect("doc-two"),
        b"bbbbbbbb\n"
    );
    assert_eq!(
        reader.read_document("missing"),
        Err(QztError::DocumentNotFound)
    );
}

#[test]
fn info_json_identity_fields_survive_document_index_container() {
    use std::fs;
    use std::process::Command;

    let input = b"doc-one\n";
    let container_id = [0x93; 16];
    let document_index = DocumentIndex {
        container_id,
        documents: vec![document(&DocumentFixture {
            doc_id: "doc-one",
            input,
            logical_offset: 0,
            byte_length: 8,
            first_line: 0,
            line_count: 1,
            chunk_start: 0,
            chunk_end: 1,
        })],
    };
    let container =
        pack_document_fixture(input, container_id, writer_options(8, 8), &document_index)
            .expect("document-index container should pack structurally");

    let base = std::env::temp_dir().join(format!("qzt-93-info-json-docidx-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let path = base.join("indexed.qzt");
    fs::write(&path, container).expect("write indexed container");

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(["info", path.to_str().unwrap(), "--format", "json"])
        .output()
        .expect("qzt info should run");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "success JSON mode must not write to stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = String::from_utf8(output.stdout).expect("stdout is utf-8");
    let value: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("stdout must be valid JSON: {error}\n{text}"));

    let object = value
        .as_object()
        .expect("stdout must be a single JSON object");

    let container_id_hex = object
        .get("container_id")
        .and_then(serde_json::Value::as_str)
        .expect("container_id must be a string");
    assert!(
        !container_id_hex.is_empty(),
        "container_id must be non-empty"
    );

    let checksum = object
        .get("original_checksum")
        .and_then(serde_json::Value::as_object)
        .expect("original_checksum must be an object");
    assert_eq!(
        checksum
            .get("algorithm")
            .and_then(serde_json::Value::as_str),
        Some("blake3")
    );
    let checksum_value = checksum
        .get("value")
        .and_then(serde_json::Value::as_str)
        .expect("original_checksum.value must be a string");
    assert!(
        !checksum_value.is_empty(),
        "original_checksum.value must be non-empty"
    );

    assert_eq!(
        object
            .get("document_index")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        object
            .get("document_count")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );

    let _ = fs::remove_dir_all(base);
}
