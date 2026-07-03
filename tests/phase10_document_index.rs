use qzt::{
    Checksum, DocumentEntry, DocumentIndex, QztError, QztReader, VerifyLevel,
    open_skeleton_details, pack_bytes_with_document_index, pack_bytes_with_memory_profile,
};
mod support;
use support::{DocumentFixture, document, writer_options};

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
        pack_bytes_with_document_index(input, [0xb0; 16], writer_options(64, 64), &document_index)
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
    let container =
        pack_bytes_with_document_index(input, [0xb1; 16], writer_options(8, 8), &document_index)
            .expect("document-index container should pack structurally");
    let reader = QztReader::open(container).expect("container should open structurally");

    assert_eq!(
        reader.verify(VerifyLevel::Deep),
        Err(QztError::ChunkTableInvalid)
    );
}

#[test]
fn memory_profile_writes_dense_and_document_index_flags_and_deep_verifies() {
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
    let container =
        pack_bytes_with_memory_profile(input, [0xb2; 16], writer_options(8, 8), &document_index)
            .expect("memory profile should pack");
    let details = open_skeleton_details(&container).expect("memory profile should open");

    assert_eq!(details.metadata.profile, "memory");
    assert!(details.metadata.dense_line_index);
    assert!(details.metadata.document_index);
    assert!(details.dense_line_index.is_some());
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
    pack_bytes_with_document_index(TWO_LINES, [0xc0; 16], writer_options(9, 9), &document_index)
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
