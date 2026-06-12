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
        &document_index,
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
    DocumentEntry::new(
        doc_id,
        0,
        input.len() as u64,
        0,
        1,
        0,
        1,
        Checksum::blake3(input),
    )
}
