use qzt::{
    IndexBuildLimits, NgramIndexBuildOptions, QziFileSidecar, QztError, QztFileReader,
    RawNgramIndex, RawTokenIndex, SearchOptions, SidecarBuildOptions, SidecarIndexKind,
    TokenIndexBuildOptions, WriterOptions, build_search_sidecar,
    build_search_sidecar_from_file_with_options, pack_bytes,
};
use std::fs;
use std::process::Command;

#[test]
fn cli_build_refusal_preserves_source_and_existing_output() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("input.qzt");
    let output = directory.path().join("existing.qzi");
    let container = pack_bytes(b"alpha alpha\nbeta\n", WriterOptions::default()).unwrap();
    fs::write(&source, &container).unwrap();
    let previous = build_search_sidecar(&container, SidecarIndexKind::Token).unwrap();
    fs::write(&output, &previous).unwrap();
    for kind in ["token", "ngram"] {
        let result = Command::new(env!("CARGO_BIN_EXE_qzt"))
            .args([
                "sidecar-rebuild",
                source.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
                "--index",
                kind,
                "--max-build-postings",
                "0",
            ])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&result.stderr).contains("a resource limit was exceeded"));
        assert_eq!(result.stdout, [] as [u8; 0]);
        assert_eq!(fs::read(&source).unwrap(), container);
        assert_eq!(fs::read(&output).unwrap(), previous);
    }
}

fn build(
    input: &[u8],
    kind: SidecarIndexKind,
    limits: IndexBuildLimits,
) -> Result<Vec<u8>, QztError> {
    let container = pack_bytes(input, WriterOptions::default()).unwrap();
    let reader = QztFileReader::open_read_at(container.as_slice(), container.len() as u64).unwrap();
    build_search_sidecar_from_file_with_options(
        &reader,
        kind,
        SidecarBuildOptions {
            limits,
            ..Default::default()
        },
    )
}

#[test]
fn empty_input_accepts_zero_retained_and_minimal_section_budget() {
    let limits = IndexBuildLimits {
        max_granules: 0,
        max_distinct_keys: 0,
        max_posting_ids: 0,
        max_key_bytes: 0,
        max_encoded_bytes: 16,
    };
    for kind in [SidecarIndexKind::Token, SidecarIndexKind::Ngram { n: 2 }] {
        build(b"", kind, limits).unwrap();
        assert_eq!(
            build(
                b"",
                kind,
                IndexBuildLimits {
                    max_encoded_bytes: 15,
                    ..limits
                }
            ),
            Err(QztError::ResourceLimitExceeded)
        );
        assert_eq!(
            build(b"\n", kind, limits),
            Err(QztError::ResourceLimitExceeded)
        );
    }
}

#[test]
fn token_retained_boundaries_deduplicate_per_line_and_fold_ascii() {
    let limits = IndexBuildLimits {
        max_granules: 2,
        max_distinct_keys: 2,
        max_posting_ids: 3,
        max_key_bytes: 4,
        ..Default::default()
    };
    let input = b"AA aa aa\r\naa bb\n";
    let expected = build(input, SidecarIndexKind::Token, limits).unwrap();
    assert_eq!(
        expected,
        build(input, SidecarIndexKind::Token, IndexBuildLimits::default()).unwrap()
    );
    for rejected in [
        IndexBuildLimits {
            max_granules: 1,
            ..limits
        },
        IndexBuildLimits {
            max_distinct_keys: 1,
            ..limits
        },
        IndexBuildLimits {
            max_posting_ids: 2,
            ..limits
        },
        IndexBuildLimits {
            max_key_bytes: 3,
            ..limits
        },
    ] {
        assert_eq!(
            build(input, SidecarIndexKind::Token, rejected),
            Err(QztError::ResourceLimitExceeded)
        );
    }
}

#[test]
fn scalar_ngram_boundaries_preserve_emoji_crlf_and_posting_order() {
    let input = "😀😀😀\r\n😀😀\r\n".as_bytes();
    // Each line: 😀😀, 😀CR, CRLF. Emoji is one scalar, four bytes.
    let limits = IndexBuildLimits {
        max_granules: 2,
        max_distinct_keys: 3,
        max_posting_ids: 6,
        max_key_bytes: 15,
        ..Default::default()
    };
    let container = pack_bytes(input, WriterOptions::default()).unwrap();
    let index = RawNgramIndex::build_from_container(
        &container,
        NgramIndexBuildOptions {
            n: 2,
            limits,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        index
            .terms
            .iter()
            .map(|term| term.key.as_slice())
            .collect::<Vec<_>>(),
        vec![b"\r\n".as_slice(), "😀\r".as_bytes(), "😀😀".as_bytes()]
    );
    assert_eq!(index.postings, vec![vec![0, 1], vec![0, 1], vec![0, 1]]);
    for rejected in [
        IndexBuildLimits {
            max_posting_ids: 5,
            ..limits
        },
        IndexBuildLimits {
            max_key_bytes: 14,
            ..limits
        },
    ] {
        assert_eq!(
            RawNgramIndex::build_from_container(
                &container,
                NgramIndexBuildOptions {
                    n: 2,
                    limits: rejected,
                    ..Default::default()
                }
            ),
            Err(QztError::ResourceLimitExceeded)
        );
    }
}

#[test]
fn encoded_boundary_and_materialized_entrance_are_bounded() {
    let container = pack_bytes(b"aa aa\nbb\n", WriterOptions::default()).unwrap();
    let limits = IndexBuildLimits {
        max_encoded_bytes: 2,
        ..Default::default()
    };
    let index = RawTokenIndex::build_from_container(
        &container,
        TokenIndexBuildOptions {
            limits,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        RawTokenIndex::build_from_container(
            &container,
            TokenIndexBuildOptions {
                limits: IndexBuildLimits {
                    max_encoded_bytes: 1,
                    ..limits
                },
                ..Default::default()
            }
        ),
        Err(QztError::ResourceLimitExceeded)
    );
    assert_eq!(
        RawTokenIndex::from_parts(
            index.container_id,
            index.source_size_bytes,
            index.granules,
            index.terms,
            index.postings,
            IndexBuildLimits {
                max_posting_ids: 1,
                ..limits
            }
        ),
        Err(QztError::ResourceLimitExceeded)
    );
    // Independent section layout: 8+2*20 granules, 8+2*(1+2+1+1) terms, 2 postings = 68.
    build(
        b"aa aa\nbb\n",
        SidecarIndexKind::Token,
        IndexBuildLimits {
            max_encoded_bytes: 68,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        build(
            b"aa aa\nbb\n",
            SidecarIndexKind::Token,
            IndexBuildLimits {
                max_encoded_bytes: 67,
                ..Default::default()
            }
        ),
        Err(QztError::ResourceLimitExceeded)
    );
}

#[test]
fn ngram_encoded_budget_includes_skip_records_at_the_exact_boundary() {
    let input = b"aa\n".repeat(1024);
    let container = pack_bytes(&input, WriterOptions::default()).unwrap();
    // Two lists of 1024 one-byte deltas, each with seven 24-byte skip records.
    let limits = IndexBuildLimits {
        max_encoded_bytes: 2384,
        ..Default::default()
    };
    let index = RawNgramIndex::build_from_container(
        &container,
        NgramIndexBuildOptions {
            n: 2,
            limits,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        RawNgramIndex::from_parts(
            index.container_id,
            index.source_size_bytes,
            index.granules,
            index.terms,
            index.postings,
            NgramIndexBuildOptions {
                n: 2,
                limits: IndexBuildLimits {
                    max_encoded_bytes: 2383,
                    ..limits
                },
                ..Default::default()
            }
        ),
        Err(QztError::ResourceLimitExceeded)
    );
}

#[test]
fn long_repetitive_line_uses_distinct_pairs_and_existing_line_limit() {
    let mut input = b"ab".repeat(32_768);
    input.push(b'\n');
    let limits = IndexBuildLimits {
        max_distinct_keys: 3,
        max_posting_ids: 3,
        max_key_bytes: 6,
        max_granules: 1,
        ..Default::default()
    };
    let container = pack_bytes(&input, WriterOptions::default()).unwrap();
    let reader = QztFileReader::open_read_at(container.as_slice(), container.len() as u64).unwrap();
    build_search_sidecar_from_file_with_options(
        &reader,
        SidecarIndexKind::Ngram { n: 2 },
        SidecarBuildOptions {
            max_line_bytes: input.len() as u64,
            limits,
        },
    )
    .unwrap();
    assert_eq!(
        build_search_sidecar_from_file_with_options(
            &reader,
            SidecarIndexKind::Ngram { n: 2 },
            SidecarBuildOptions {
                max_line_bytes: input.len() as u64 - 1,
                limits
            }
        ),
        Err(QztError::ResourceLimitExceeded)
    );
}

#[test]
fn cli_invalid_limits_precede_io_and_rejection_leaves_no_new_output() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("absent.qzt");
    let new_output = directory.path().join("new.qzi");
    for flag in [
        "--max-build-granules",
        "--max-build-keys",
        "--max-build-postings",
        "--max-build-key-bytes",
        "--max-build-encoded-bytes",
    ] {
        for value in ["-1", "bad", "18446744073709551616"] {
            for args in [
                vec![
                    "sidecar-rebuild",
                    missing.to_str().unwrap(),
                    "-o",
                    new_output.to_str().unwrap(),
                    flag,
                    value,
                ],
                vec!["search", missing.to_str().unwrap(), "aa", flag, value],
            ] {
                let result = Command::new(env!("CARGO_BIN_EXE_qzt"))
                    .args(args)
                    .output()
                    .unwrap();
                assert_eq!(result.status.code(), Some(2));
                assert!(!new_output.exists());
            }
        }
    }
    let source = directory.path().join("input.qzt");
    let container = pack_bytes(b"aa\n", WriterOptions::default()).unwrap();
    fs::write(&source, &container).unwrap();
    for kind in ["token", "ngram"] {
        let result = Command::new(env!("CARGO_BIN_EXE_qzt"))
            .args([
                "sidecar-rebuild",
                source.to_str().unwrap(),
                "-o",
                new_output.to_str().unwrap(),
                "--index",
                kind,
                "--max-build-granules",
                "0",
            ])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(!new_output.exists());
        assert_eq!(fs::read(&source).unwrap(), container);
        let result = Command::new(env!("CARGO_BIN_EXE_qzt"))
            .args([
                "search",
                source.to_str().unwrap(),
                "aa",
                "--index",
                kind,
                "--max-build-keys",
                "0",
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert_eq!(result.stdout, [] as [u8; 0]);
    }
    let result = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args([
            "search",
            missing.to_str().unwrap(),
            "aa",
            "--sidecar",
            "absent.qzi",
            "--max-build-keys",
            "3",
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
}

#[test]
fn sufficient_limits_match_default_bytes_and_file_search_contract() {
    let input = "aa aa\r\n東京😀 aa\nmissing\n".as_bytes();
    let container = pack_bytes(input, WriterOptions::default()).unwrap();
    let reader = QztFileReader::open_read_at(container.as_slice(), container.len() as u64).unwrap();
    for kind in [SidecarIndexKind::Token, SidecarIndexKind::Ngram { n: 2 }] {
        let default = build_search_sidecar(&container, kind).unwrap();
        let selected = build_search_sidecar_from_file_with_options(
            &reader,
            kind,
            SidecarBuildOptions {
                limits: IndexBuildLimits {
                    max_granules: 3,
                    max_distinct_keys: 100,
                    max_posting_ids: 100,
                    max_key_bytes: 1000,
                    max_encoded_bytes: 1000,
                },
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(selected, default);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("index.qzi");
        fs::write(&path, selected).unwrap();
        let sidecar = QziFileSidecar::open_path(&path, &reader).unwrap();
        let report = sidecar
            .search(&reader, "aa", SearchOptions::default())
            .unwrap();
        assert_eq!(report.hits.len(), 3);
        assert!(!report.capped);
        assert!(!report.index_coverage_verified);
        for hit in report.hits {
            let offset = usize::try_from(hit.logical_offset).unwrap();
            let length = usize::try_from(hit.byte_length).unwrap();
            assert_eq!(&input[offset..offset + length], b"aa");
        }
    }
}

fn unhex(text: &str) -> Vec<u8> {
    text.trim()
        .as_bytes()
        .chunks_exact(2)
        .map(|bytes| u8::from_str_radix(std::str::from_utf8(bytes).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn unicode_crlf_cross_chunk_qzi_bytes_match_frozen_before_build() {
    let source = unhex(include_str!("fixtures/index-build-budget/source.qzt.hex"));
    for (kind, frozen) in [
        (
            SidecarIndexKind::Token,
            include_str!("fixtures/index-build-budget/token.qzi.hex"),
        ),
        (
            SidecarIndexKind::Ngram { n: 2 },
            include_str!("fixtures/index-build-budget/ngram.qzi.hex"),
        ),
    ] {
        let bytes = build_search_sidecar(&source, kind).unwrap();
        assert_eq!(bytes, unhex(frozen));
        qzt::QziSidecar::open(&source, &bytes).unwrap();
    }
}
