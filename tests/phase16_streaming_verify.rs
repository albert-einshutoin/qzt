use qzt::chunker::ChunkerOptions;
use qzt::reader::{QztFileReader, QztReader, VerifyLevel};
use qzt::schema::{Checksum, DocumentEntry, DocumentIndex};
use qzt::writer::{pack_bytes_with_document_index, pack_bytes_with_profile, WriterOptions};

fn options() -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size: 8,
            max_chunk_size: 8,
        },
        zstd_level: 0,
    }
}

#[test]
fn file_backed_deep_verify_matches_in_memory_deep_verify() {
    let input = b"alpha\nbeta\ngamma\nlong line continues across chunks\n";
    let container = pack_bytes_with_profile(input, options(), "memory", true)
        .expect("memory profile should pack");
    let memory = QztReader::open(&container).expect("memory reader should open");
    let file =
        QztFileReader::open_read_at(&container[..], container.len() as u64).expect("file open");

    assert_eq!(
        file.verify(VerifyLevel::Deep),
        memory.verify(VerifyLevel::Deep)
    );
}

#[test]
fn deep_verify_rejects_stale_document_index_with_range_scoped_read() {
    let input = b"doc-one\ndoc-two\n";
    let document_index = DocumentIndex {
        container_id: [0x61; 16],
        documents: vec![document(DocumentFixture {
            doc_id: "doc-one",
            input,
            logical_offset: 0,
            byte_length: 8,
            first_line: 0,
            line_count: 1,
            chunk_start: 0,
            chunk_end: 1,
            checksum_bytes: b"wrong",
        })],
    };
    let container = pack_bytes_with_document_index(input, [0x61; 16], options(), document_index)
        .expect("document-index container should pack");
    let file =
        QztFileReader::open_read_at(&container[..], container.len() as u64).expect("file open");

    assert_eq!(
        file.verify(VerifyLevel::Deep),
        Err(qzt::error::QztError::ContainerCorrupt)
    );
}

#[test]
fn export_to_streams_chunk_order_without_materializing_api_requirement() {
    let input = b"chunk-a\nchunk-b\nchunk-c\nchunk-d\n";
    let container =
        pack_bytes_with_profile(input, options(), "core", false).expect("container should pack");
    let file =
        QztFileReader::open_read_at(&container[..], container.len() as u64).expect("file open");
    let mut output = Vec::new();

    file.export_to(&mut output).expect("export should work");

    assert_eq!(output, input);
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
    checksum_bytes: &'a [u8],
}

fn document(fixture: DocumentFixture<'_>) -> DocumentEntry {
    let hash = blake3::hash(fixture.doc_id.as_bytes());
    let mut doc_id_hash = [0_u8; 16];
    doc_id_hash.copy_from_slice(&hash.as_bytes()[..16]);
    let fallback_end = usize::try_from(fixture.logical_offset)
        .ok()
        .and_then(|start| start.checked_add(usize::try_from(fixture.byte_length).ok()?))
        .unwrap_or(fixture.input.len());
    let range = fixture
        .input
        .get(
            usize::try_from(fixture.logical_offset).unwrap_or(0)
                ..fallback_end.min(fixture.input.len()),
        )
        .unwrap_or(&[]);

    DocumentEntry {
        doc_id: fixture.doc_id.to_owned(),
        doc_id_hash,
        logical_offset: fixture.logical_offset,
        byte_length: fixture.byte_length,
        first_line: fixture.first_line,
        line_count: fixture.line_count,
        chunk_start: fixture.chunk_start,
        chunk_end: fixture.chunk_end,
        checksum: if fixture.checksum_bytes == b"actual" {
            Checksum::blake3(range)
        } else {
            Checksum::blake3(fixture.checksum_bytes)
        },
    }
}
