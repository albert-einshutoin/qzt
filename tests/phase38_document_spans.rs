use qzt::{
    ChunkerOptions, DocumentEntry, DocumentIndex, DocumentSpan, QztReader, VerifyLevel,
    WriterBuilder, WriterOptions,
};

fn writer_options() -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size: 9,
            max_chunk_size: 9,
        },
        zstd_level: 0,
    }
}

#[test]
fn document_spans_build_entries_for_single_cross_chunk_and_empty_documents() {
    let input = b"alpha\nbeta\ngamma\n";
    let container = WriterBuilder::new()
        .container_id([0x38; 16])
        .options(writer_options())
        .document_spans(vec![
            DocumentSpan::new("single", 0, 6),
            DocumentSpan::new("cross", 6, 11),
            DocumentSpan::new("mid-line", 2, 7),
            DocumentSpan::new("empty", 17, 0),
        ])
        .pack(input)
        .expect("document spans should pack");
    let reader = QztReader::open(&container).expect("container should open");
    let details = qzt::open_skeleton_details(&container).expect("container details should open");
    let index = details
        .document_index
        .as_ref()
        .expect("document index should be present");

    assert_eq!(index.documents[0].doc_id, "single");
    assert_eq!(index.documents[0].first_line, 0);
    assert_eq!(index.documents[0].line_count, 1);
    assert_eq!(
        (index.documents[0].chunk_start, index.documents[0].chunk_end),
        (0, 1)
    );
    assert_eq!(
        reader.read_document_verified("single", &index.documents[0].checksum),
        Ok(b"alpha\n".to_vec())
    );

    assert_eq!(index.documents[1].doc_id, "cross");
    assert_eq!(index.documents[1].first_line, 1);
    assert_eq!(index.documents[1].line_count, 2);
    assert_eq!(
        (index.documents[1].chunk_start, index.documents[1].chunk_end),
        (1, 3)
    );
    assert_eq!(
        reader.read_document_verified("cross", &index.documents[1].checksum),
        Ok(b"beta\ngamma\n".to_vec())
    );

    assert_eq!(index.documents[2].first_line, 0);
    assert_eq!(index.documents[2].line_count, 2);
    assert_eq!(
        (index.documents[2].chunk_start, index.documents[2].chunk_end),
        (0, 2)
    );
    assert_eq!(
        reader.read_document_verified("mid-line", &index.documents[2].checksum),
        Ok(b"pha\nbet".to_vec())
    );

    assert_eq!(index.documents[3].first_line, 3);
    assert_eq!(index.documents[3].line_count, 0);
    assert_eq!(
        (index.documents[3].chunk_start, index.documents[3].chunk_end),
        (0, 0)
    );
    assert_eq!(
        reader.read_document_verified("empty", &index.documents[3].checksum),
        Ok(Vec::new())
    );
    assert!(reader.verify(VerifyLevel::Deep).is_ok());
}

#[test]
fn document_spans_reject_duplicate_doc_ids_and_out_of_range_spans() {
    let duplicate = WriterBuilder::new()
        .document_spans(vec![
            DocumentSpan::new("same", 0, 1),
            DocumentSpan::new("same", 1, 1),
        ])
        .pack(b"ab");
    assert_eq!(duplicate, Err(qzt::QztError::DuplicateDocumentId));
    assert!(
        duplicate
            .unwrap_err()
            .to_string()
            .contains("duplicate document id")
    );

    let out_of_range = WriterBuilder::new()
        .document_spans(vec![DocumentSpan::new("bad", 1, 2)])
        .pack(b"ab");
    assert_eq!(out_of_range, Err(qzt::QztError::LogicalRangeOutOfBounds));
}

#[test]
fn supplied_document_indexes_reject_duplicate_doc_ids() {
    let duplicate_index = DocumentIndex {
        container_id: [0x39; 16],
        documents: vec![
            DocumentEntry::new("same", 0, 1, 0, 1, 0, 1, qzt::Checksum::blake3(b"a")),
            DocumentEntry::new("same", 1, 1, 1, 1, 1, 2, qzt::Checksum::blake3(b"b")),
        ],
    };

    let result = WriterBuilder::new()
        .container_id([0x39; 16])
        .document_index(duplicate_index)
        .pack(b"a\nb\n");
    assert_eq!(result, Err(qzt::QztError::DuplicateDocumentId));
}

#[test]
fn memory_profile_accepts_document_spans() {
    let container = WriterBuilder::new()
        .profile("memory")
        .document_spans(vec![DocumentSpan::new("all", 0, 6)])
        .pack(b"hello\n")
        .expect("memory profile should accept generated document index");
    let reader = QztReader::open(&container).expect("container should open");
    assert!(reader.verify(VerifyLevel::Deep).is_ok());
}
