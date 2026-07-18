use qzt::{
    Checksum, ChunkerOptions, DocumentEntry, DocumentIndex, QztError, QztFileReader, QztReader,
    VerifyLevel, WriterBuilder, WriterOptions,
};

const C1_DOC_ID: &str = "conversation_c1_validation_smoke";

/// Deterministic C1 conversation fixture for Phase23a smoke coverage.
///
/// Includes multi-turn dialogue, markdown-like headings/lists, a fenced code
/// block, and UTF-8 multibyte content (emoji + Japanese). No RNG — byte output
/// is stable across runs and toolchains.
fn c1_validation_corpus_fixture() -> Vec<u8> {
    concat!(
        "# QZT C1 validation smoke fixture\n",
        "\n",
        "## Turn 1\n",
        "user: Explain how doc_id works in evidence retrieval.\n",
        "assistant:\n",
        "- cite by doc_id\n",
        "- verify blake3 checksums\n",
        "- restore byte-exact ranges\n",
        "\n",
        "## Turn 2\n",
        "user: Show a Rust snippet.\n",
        "assistant:\n",
        "```rust\n",
        "fn evidence_ref(doc_id: &str) -> Result<Vec<u8>> {\n",
        "    reader.read_document_verified(doc_id, &checksum)\n",
        "}\n",
        "```\n",
        "\n",
        "## Turn 3\n",
        "user: UTF-8 boundary check 🇯🇵 日本語テキスト\n",
        "assistant: emojiと漢字の混在をバイト単位で保持します。\n",
    )
    .as_bytes()
    .to_vec()
}

fn line_count_for(input: &[u8]) -> u64 {
    if input.is_empty() {
        return 0;
    }
    #[allow(clippy::naive_bytecount)]
    let mut lines = input.iter().filter(|&&b| b == b'\n').count() as u64;
    if !input.ends_with(b"\n") {
        lines += 1;
    }
    lines
}

fn c1_document_index(corpus: &[u8]) -> DocumentIndex {
    DocumentIndex {
        container_id: [0xC1; 16],
        documents: vec![DocumentEntry::new(
            C1_DOC_ID,
            0,
            u64::try_from(corpus.len()).expect("fixture fits in u64"),
            0,
            line_count_for(corpus),
            // The writer options below keep this smoke fixture in one chunk,
            // so the document's chunk interval is deterministically 0..1.
            0,
            1,
            Checksum::blake3(corpus),
        )],
    }
}

#[test]
fn c1_fixture_has_conversation_shape_and_is_deterministic() {
    let first = c1_validation_corpus_fixture();
    let second = c1_validation_corpus_fixture();

    assert_eq!(first, second, "C1 fixture must be deterministic");
    let text = std::str::from_utf8(&first).expect("fixture is valid UTF-8");

    assert!(text.contains("## Turn 1"), "multi-turn dialogue");
    assert!(text.contains("## Turn 2"), "multi-turn dialogue");
    assert!(text.contains("## Turn 3"), "multi-turn dialogue");
    assert!(text.contains("# QZT C1"), "markdown-like heading");
    assert!(text.contains("- cite by doc_id"), "markdown-like list");
    assert!(text.contains("```rust"), "code fence");
    assert!(text.contains("日本語テキスト"), "UTF-8 multibyte content");
    assert!(text.contains("🇯🇵"), "UTF-8 emoji");
}

#[test]
fn c1_conversation_pack_export_document_index_and_utf8_boundary_smoke() {
    let corpus = c1_validation_corpus_fixture();
    let document_index = c1_document_index(&corpus);
    // One chunk keeps DocumentIndex chunk_start/chunk_end stable for this test.
    let chunk_size = corpus.len();
    let writer_options = WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size: chunk_size,
            max_chunk_size: chunk_size,
        },
        zstd_level: 3,
    };
    let container = WriterBuilder::new()
        .container_id(document_index.container_id)
        .options(writer_options)
        .document_index(document_index)
        .pack(&corpus)
        .expect("pack should succeed");

    let reader = QztReader::open(&container).expect("reader should open");
    assert_eq!(
        reader.export_all().expect("export"),
        corpus,
        "byte-exact pack/export round trip"
    );
    assert!(reader.verify(VerifyLevel::Deep).is_ok(), "deep verify");

    let expected = Checksum::blake3(&corpus);
    assert_eq!(
        reader.read_document(C1_DOC_ID).expect("read_document"),
        corpus,
        "doc_id resolves to original bytes"
    );
    assert_eq!(
        reader
            .read_document_verified(C1_DOC_ID, &expected)
            .expect("read_document_verified"),
        corpus,
        "verified doc_id restore"
    );

    let file = QztFileReader::open_read_at(&container[..], container.len() as u64)
        .expect("file reader should open");
    assert_eq!(
        file.read_document(C1_DOC_ID).expect("file read_document"),
        corpus
    );

    let utf8_line = "user: UTF-8 boundary check 🇯🇵 日本語テキスト\n";

    let jp_needle = "日本語";
    let jp_offset = corpus
        .windows(jp_needle.len())
        .position(|window| window == jp_needle.as_bytes())
        .expect("Japanese text is present") as u64;

    assert_eq!(
        reader.read_range(jp_offset, 3).expect("aligned byte range"),
        "日".as_bytes(),
        "range read on UTF-8 scalar boundary"
    );
    assert_eq!(
        reader.read_text_range(jp_offset, 3),
        Ok("日".to_owned()),
        "UTF-8-safe text range on aligned boundary"
    );
    assert_eq!(
        reader.read_text_range(jp_offset + 1, 2),
        Err(QztError::InvalidUtf8Boundary),
        "misaligned range must fail closed"
    );

    let utf8_line_index = corpus
        .split_inclusive(|&b| b == b'\n')
        .position(|line| line == utf8_line.as_bytes())
        .expect("UTF-8 line index") as u64;
    assert_eq!(
        reader.read_line_raw(utf8_line_index).expect("line read"),
        utf8_line.as_bytes(),
        "line read through multibyte UTF-8 content"
    );
}
