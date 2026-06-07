use qzt::chunker::ChunkerOptions;
use qzt::error::QztError;
use qzt::reader::{QztReader, VerifyLevel};
use qzt::schema::{Checksum, DocumentEntry, DocumentIndex};
use qzt::skeleton::open_skeleton_details;
use qzt::writer::{pack_bytes_with_document_index, pack_bytes_with_memory_profile, WriterOptions};

fn options(target_chunk_size: usize, max_chunk_size: usize) -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size,
            max_chunk_size,
        },
        zstd_level: 0,
    }
}

#[test]
fn document_index_ranges_are_verified_by_deep_verify() {
    let input = b"doc-one\n";
    let document_index = DocumentIndex {
        container_id: [0xb0; 16],
        documents: vec![document(DocumentFixture {
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
        pack_bytes_with_document_index(input, [0xb0; 16], options(64, 64), document_index)
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
        documents: vec![document(DocumentFixture {
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
        pack_bytes_with_document_index(input, [0xb1; 16], options(8, 8), document_index)
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
            document(DocumentFixture {
                doc_id: "doc-one",
                input,
                logical_offset: 0,
                byte_length: 8,
                first_line: 0,
                line_count: 1,
                chunk_start: 0,
                chunk_end: 1,
            }),
            document(DocumentFixture {
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
        pack_bytes_with_memory_profile(input, [0xb2; 16], options(8, 8), document_index)
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

struct DocumentFixture<'a> {
    doc_id: &'a str,
    input: &'a [u8],
    logical_offset: u64,
    byte_length: u64,
    first_line: u64,
    line_count: u64,
    chunk_start: u64,
    chunk_end: u64,
}

fn document(fixture: DocumentFixture<'_>) -> DocumentEntry {
    let start = usize::try_from(fixture.logical_offset).unwrap_or(fixture.input.len());
    let end = start
        .checked_add(usize::try_from(fixture.byte_length).unwrap_or(0))
        .unwrap_or(start)
        .min(fixture.input.len());
    let range = fixture.input.get(start..end).unwrap_or(&[]);
    let hash = blake3::hash(fixture.doc_id.as_bytes());
    let mut doc_id_hash = [0_u8; 16];
    doc_id_hash.copy_from_slice(&hash.as_bytes()[..16]);

    DocumentEntry {
        doc_id: fixture.doc_id.to_owned(),
        doc_id_hash,
        logical_offset: fixture.logical_offset,
        byte_length: fixture.byte_length,
        first_line: fixture.first_line,
        line_count: fixture.line_count,
        chunk_start: fixture.chunk_start,
        chunk_end: fixture.chunk_end,
        checksum: Checksum::blake3(range),
    }
}
