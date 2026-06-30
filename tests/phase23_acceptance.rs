use qzt::corpus::{CorpusKind, ValidationCorpusOptions, generate_validation_corpus};
use qzt::reader::{QztFileReader, QztReader, VerifyLevel};
use qzt::search::{RawTokenIndex, SearchOptions, TokenIndexBuildOptions};
use qzt::skeleton::open_skeleton_details;
use qzt::writer::pack_bytes;
mod support;
use support::writer_options;

#[test]
fn validation_corpora_are_deterministic() {
    for kind in CorpusKind::all() {
        let opts = ValidationCorpusOptions {
            seed: 42,
            target_bytes: 4096,
        };
        assert_eq!(
            generate_validation_corpus(kind, opts),
            generate_validation_corpus(kind, opts),
            "{} should be deterministic",
            kind.id()
        );
    }
}

#[test]
fn hard_invariants_hold_for_c1_through_c6() {
    for kind in CorpusKind::all() {
        let corpus = generate_validation_corpus(
            kind,
            ValidationCorpusOptions {
                seed: 7,
                target_bytes: 16 * 1024,
            },
        )
        .expect("corpus should generate");
        let container = pack_bytes(&corpus, writer_options(1024, 1024)).expect("pack should work");
        let reader = QztReader::open(&container).expect("reader should open");
        assert_eq!(
            reader.export_all().expect("export"),
            corpus,
            "{}",
            kind.id()
        );
        assert!(reader.verify(VerifyLevel::Deep).is_ok(), "{}", kind.id());

        let offset = (corpus.len() / 3) as u64;
        let length = 512_u64.min(corpus.len().saturating_sub(corpus.len() / 3) as u64);
        let file =
            QztFileReader::open_read_at(&container[..], container.len() as u64).expect("file open");
        let start = usize::try_from(offset).expect("offset fits in usize in tests");
        let end = usize::try_from(offset + length).expect("offset+length fits in usize in tests");
        assert_eq!(
            file.read_range(offset, length).expect("range"),
            corpus[start..end],
            "{}",
            kind.id()
        );
    }
}

#[test]
fn corruption_sweep_detects_chunk_metadata_and_index_mutations() {
    let corpus = generate_validation_corpus(
        CorpusKind::C2Logs,
        ValidationCorpusOptions {
            seed: 99,
            target_bytes: 8192,
        },
    )
    .expect("corpus should generate");
    let container = pack_bytes(&corpus, writer_options(1024, 1024)).expect("pack should work");
    let details = open_skeleton_details(&container).expect("details");
    let first_chunk = details.chunk_entries.first().expect("chunk");

    for offset in [
        first_chunk.physical_offset,
        details.footer_payload.metadata.offset,
        details.footer_payload.index_root.offset,
    ] {
        let mut corrupt = container.clone();
        corrupt[usize::try_from(offset).expect("offset fits in usize in tests")] ^= 0x55;
        assert!(
            QztReader::open(&corrupt)
                .and_then(|reader| reader.verify(VerifyLevel::Deep))
                .is_err(),
            "corruption at {offset} should be detected"
        );
    }
}

#[test]
fn soft_targets_are_recorded_without_hard_failing_on_band_drift() {
    let corpus = generate_validation_corpus(
        CorpusKind::C1Conversation,
        ValidationCorpusOptions {
            seed: 123,
            target_bytes: 32 * 1024,
        },
    )
    .expect("corpus should generate");
    let container = pack_bytes(&corpus, writer_options(1024, 1024)).expect("pack should work");
    let reader = QztReader::open(&container).expect("reader");
    let index = RawTokenIndex::build_from_container(&container, TokenIndexBuildOptions::default())
        .expect("token index");
    let report = index
        .search(
            &reader,
            "evidence",
            SearchOptions {
                max_candidate_granules: 10_000,
                max_decoded_bytes: 2 * 1024 * 1024,
                max_search_results: 16,
            },
        )
        .expect("search");
    eprintln!(
        "phase23_soft corpus=C1 source_bytes={} container_bytes={} compression_ratio={:.6} decoded_ratio={:.6} capped={}",
        corpus.len(),
        container.len(),
        container.len() as f64 / corpus.len() as f64,
        report.metrics.decoded_bytes as f64 / corpus.len() as f64,
        report.capped
    );
}
