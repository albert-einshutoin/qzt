use qzt::{pack_bytes_with_document_index, Checksum, DocumentEntry, DocumentIndex, WriterOptions};
use qzt::{QztFileReader, WriterBuilder};

fn main() -> qzt::Result<()> {
    let input = b"doc-one evidence line\n";
    let document_index = DocumentIndex {
        container_id: [0x42; 16],
        documents: vec![document("doc-one", input)],
    };
    let container = pack_bytes_with_document_index(
        input,
        [0x42; 16],
        WriterOptions::default(),
        document_index,
    )?;
    let reader = QztFileReader::open_read_at(&container[..], container.len() as u64)?;
    let expected = Checksum::blake3(b"evidence");
    let restored = reader.read_range_verified(8, 8, &expected)?;
    assert_eq!(restored, b"evidence");
    let doc = reader.read_document_verified("doc-one", &Checksum::blake3(input))?;
    assert_eq!(doc, input);

    let builder_container = WriterBuilder::new().pack(input)?;
    assert!(!builder_container.is_empty());
    Ok(())
}

fn document(doc_id: &str, input: &[u8]) -> DocumentEntry {
    let hash = blake3::hash(doc_id.as_bytes());
    let mut doc_id_hash = [0_u8; 16];
    doc_id_hash.copy_from_slice(&hash.as_bytes()[..16]);
    DocumentEntry {
        doc_id: doc_id.to_owned(),
        doc_id_hash,
        logical_offset: 0,
        byte_length: input.len() as u64,
        first_line: 0,
        line_count: 1,
        chunk_start: 0,
        chunk_end: 1,
        checksum: Checksum::blake3(input),
    }
}
