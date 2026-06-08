use std::thread;

use qzt::chunker::ChunkerOptions;
use qzt::error::QztError;
use qzt::reader::{QztFileReader, QztReader};
use qzt::schema::{Checksum, DocumentEntry, DocumentIndex};
use qzt::writer::{pack_bytes_with_document_index, WriterOptions};

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
fn verified_range_read_returns_bytes_or_fails_closed() {
    let input = b"alpha\nbeta\ngamma\n";
    let container = qzt::writer::pack_bytes(input, options()).expect("pack");
    let reader = QztReader::open(&container).expect("reader");
    let expected = Checksum::blake3(b"beta\n");
    let wrong = Checksum::blake3(b"wrong");

    assert_eq!(
        reader
            .read_range_verified(6, 5, &expected)
            .expect("verified"),
        b"beta\n"
    );
    assert_eq!(
        reader.read_range_verified(6, 5, &wrong),
        Err(QztError::VerifiedChecksumMismatch)
    );
}

#[test]
fn verified_document_read_resolves_document_index() {
    let input = b"doc-one\ndoc-two\n";
    let document_index = DocumentIndex {
        container_id: [0x21; 16],
        documents: vec![
            document(DocFixture {
                doc_id: "doc-one",
                input,
                logical_offset: 0,
                byte_length: 8,
                first_line: 0,
                line_count: 1,
                chunk_start: 0,
                chunk_end: 1,
            }),
            document(DocFixture {
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
        pack_bytes_with_document_index(input, [0x21; 16], options(), document_index).expect("pack");
    let memory = QztReader::open(&container).expect("memory reader");
    let file =
        QztFileReader::open_read_at(&container[..], container.len() as u64).expect("file reader");

    assert_eq!(memory.read_document("doc-two").expect("doc"), b"doc-two\n");
    assert_eq!(
        file.read_document_verified("doc-one", &Checksum::blake3(b"doc-one\n"))
            .expect("verified doc"),
        b"doc-one\n"
    );
}

#[test]
fn concurrent_file_backed_verified_reads_match_serial_reads() {
    let input = b"alpha\nbeta\ngamma\ndelta\n";
    let container = qzt::writer::pack_bytes(input, options()).expect("pack");
    let reader =
        QztFileReader::open_read_at(&container[..], container.len() as u64).expect("file reader");
    let expected = Checksum::blake3(b"beta\n");
    let serial = reader
        .read_range_verified(6, 5, &expected)
        .expect("serial verified");

    thread::scope(|scope| {
        let first = scope.spawn(|| reader.read_range_verified(6, 5, &expected));
        let second = scope.spawn(|| reader.read_range_verified(6, 5, &expected));
        assert_eq!(first.join().expect("thread"), Ok(serial.clone()));
        assert_eq!(second.join().expect("thread"), Ok(serial));
    });
}

struct DocFixture<'a> {
    doc_id: &'a str,
    input: &'a [u8],
    logical_offset: u64,
    byte_length: u64,
    first_line: u64,
    line_count: u64,
    chunk_start: u64,
    chunk_end: u64,
}

fn document(fixture: DocFixture<'_>) -> DocumentEntry {
    let hash = blake3::hash(fixture.doc_id.as_bytes());
    let mut doc_id_hash = [0_u8; 16];
    doc_id_hash.copy_from_slice(&hash.as_bytes()[..16]);
    let start = fixture.logical_offset as usize;
    let end = start + fixture.byte_length as usize;

    DocumentEntry {
        doc_id: fixture.doc_id.to_owned(),
        doc_id_hash,
        logical_offset: fixture.logical_offset,
        byte_length: fixture.byte_length,
        first_line: fixture.first_line,
        line_count: fixture.line_count,
        chunk_start: fixture.chunk_start,
        chunk_end: fixture.chunk_end,
        checksum: Checksum::blake3(&fixture.input[start..end]),
    }
}
