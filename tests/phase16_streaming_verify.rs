use qzt::reader::{QztFileReader, QztReader, VerifyLevel};
use qzt::schema::{Checksum, DocumentEntry, DocumentIndex};
use qzt::writer::{
    pack_bytes_with_document_index, pack_bytes_with_memory_profile, pack_bytes_with_profile,
};
mod support;
use support::{document_with_checksum, writer_options};

#[test]
fn file_backed_deep_verify_matches_in_memory_deep_verify() {
    // The memory profile requires a DocumentIndex; provide a minimal one that
    // covers the entire input as a single document.
    let input = b"alpha\nbeta\ngamma\nlong line continues across chunks\n";
    // count newlines; small test slice so naive bytecount is acceptable
    #[allow(clippy::naive_bytecount)]
    let line_count = input.iter().filter(|&&b| b == b'\n').count() as u64;
    let document_index = DocumentIndex {
        container_id: [0x16; 16],
        documents: vec![DocumentEntry::new(
            "all",
            0,
            input.len() as u64,
            0,
            line_count,
            0,
            // chunk count: ceil(len / max_chunk_size) = ceil(51/8) = 7
            7,
            Checksum::blake3(input),
        )],
    };
    let container =
        pack_bytes_with_memory_profile(input, [0x16; 16], writer_options(8, 8), &document_index)
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
        documents: vec![document_with_checksum(
            &support::DocumentFixtureWithChecksum {
                doc_id: "doc-one",
                input,
                logical_offset: 0,
                byte_length: 8,
                first_line: 0,
                line_count: 1,
                chunk_start: 0,
                chunk_end: 1,
                checksum_bytes: b"wrong",
            },
        )],
    };
    let container =
        pack_bytes_with_document_index(input, [0x61; 16], writer_options(8, 8), &document_index)
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
    let container = pack_bytes_with_profile(input, writer_options(8, 8), "core", false)
        .expect("container should pack");
    let file =
        QztFileReader::open_read_at(&container[..], container.len() as u64).expect("file open");
    let mut output = Vec::new();

    file.export_to(&mut output).expect("export should work");

    assert_eq!(output, input);
}
