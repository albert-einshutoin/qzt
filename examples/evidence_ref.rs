use qzt::{Checksum, DocumentSpan, QztFileReader, WriterBuilder};

fn main() -> qzt::Result<()> {
    let input = b"doc-one evidence line\n";
    let container = WriterBuilder::new()
        .container_id([0x42; 16])
        .document_spans(vec![DocumentSpan::new("doc-one", 0, input.len() as u64)])
        .pack(input)?;
    let reader = QztFileReader::open_read_at(&container[..], container.len() as u64)?;
    let expected = Checksum::blake3(b"evidence");
    let restored = reader.read_range_verified(8, 8, &expected)?;
    assert_eq!(restored, b"evidence");
    let doc = reader.read_document_verified("doc-one", &Checksum::blake3(input))?;
    assert_eq!(doc, input);

    Ok(())
}
