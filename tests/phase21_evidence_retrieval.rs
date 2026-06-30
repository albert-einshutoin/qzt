use std::thread;

use qzt::error::QztError;
use qzt::reader::{QztFileReader, QztReader};
use qzt::schema::{Checksum, DocumentIndex};
use qzt::writer::pack_bytes_with_document_index;
mod support;
use support::{document, writer_options};

#[test]
fn verified_range_read_returns_bytes_or_fails_closed() {
    let input = b"alpha\nbeta\ngamma\n";
    let container = qzt::writer::pack_bytes(input, writer_options(8, 8)).expect("pack");
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
            document(&support::DocumentFixture {
                doc_id: "doc-one",
                input,
                logical_offset: 0,
                byte_length: 8,
                first_line: 0,
                line_count: 1,
                chunk_start: 0,
                chunk_end: 1,
            }),
            document(&support::DocumentFixture {
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
        pack_bytes_with_document_index(input, [0x21; 16], writer_options(8, 8), &document_index)
            .expect("pack");
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
    let container = qzt::writer::pack_bytes(input, writer_options(8, 8)).expect("pack");
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
