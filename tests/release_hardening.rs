use qzt::benchmark::{
    ReleaseBenchmarkOptions, ReleaseBenchmarkReport, run_release_benchmark,
    run_release_benchmark_with_corpus,
};

#[test]
fn release_benchmark_reports_reproducible_large_corpus_metrics() {
    let report = run_release_benchmark(ReleaseBenchmarkOptions {
        line_count: 24_000,
        ..ReleaseBenchmarkOptions::default()
    })
    .expect("release benchmark should run");

    assert_release_benchmark_report(&report);
    eprintln!("{report}");
}

#[test]
#[ignore = "Profiling run. Execute with `make bench-profile`"]
fn release_benchmark_profile() {
    let report = run_release_benchmark(ReleaseBenchmarkOptions {
        query_repetitions: env_usize("QZT_RELEASE_BENCH_QUERY_REPETITIONS", 500),
        query_warmup_repetitions: env_usize("QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS", 20),
        ..ReleaseBenchmarkOptions::default()
    })
    .expect("release benchmark profile should run");

    assert_release_benchmark_report(&report);
    eprintln!(
        "release_benchmark_profile query_repetitions={} query_warmup_repetitions={}",
        report.query_repetitions, report.query_warmup_repetitions
    );
    eprintln!("{report}");
}

#[test]
#[ignore = "Profiling run. Execute with `make bench-profile-matrix`"]
fn release_benchmark_profile_matrix() {
    const CORPUS_SIZES: [(&str, usize); 3] = [
        ("1MB", 1_000_000),
        ("10MB", 10_000_000),
        ("100MB", 100_000_000),
    ];
    const CORPUS_KINDS: [MatrixCorpusKind; 3] = [
        MatrixCorpusKind::Ascii,
        MatrixCorpusKind::Utf8Mixed,
        MatrixCorpusKind::Japanese,
    ];

    let query_repetitions = env_usize("QZT_RELEASE_BENCH_QUERY_REPETITIONS", 500);
    let query_warmup_repetitions = env_usize("QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS", 20);

    for (corpus_label, corpus_bytes) in CORPUS_SIZES {
        for kind in CORPUS_KINDS {
            let (corpus, line_count) = build_profile_corpus(corpus_bytes, kind);
            let report = run_release_benchmark_with_corpus(
                &corpus,
                ReleaseBenchmarkOptions {
                    line_count,
                    query_repetitions,
                    query_warmup_repetitions,
                    ..ReleaseBenchmarkOptions::default()
                },
            )
            .expect("release benchmark matrix profile should run");

            assert_release_benchmark_report(&report);
            eprintln!(
                "[profile-matrix] corpus={corpus_label} kind={} lines={} bytes={} reps={} warmup={}",
                kind.label(),
                report.line_count,
                report.corpus_bytes,
                report.query_repetitions,
                report.query_warmup_repetitions,
            );
            eprintln!("{report}");
        }
    }
}

fn assert_release_benchmark_report(report: &ReleaseBenchmarkReport) {
    assert!(report.corpus_bytes >= 1_000_000);
    assert_eq!(report.exported_bytes, report.corpus_bytes);
    assert_eq!(report.rare_token_verified_matches, 1);
    assert!(report.rare_token_decoded_bytes < report.raw_scan_decoded_bytes);
    assert_eq!(report.common_ngram_query.verified_matches, 0);
    assert_eq!(report.common_ngram_query.decoded_bytes, 0);
    assert!(report.common_ngram_query.capped);
    assert_eq!(report.missing_token_query.verified_matches, 0);
    assert_eq!(report.missing_token_query.decoded_bytes, 0);
    assert_eq!(report.missing_token_query.candidate_granules, 0);
    assert_eq!(
        report.common_ngram_query.candidate_granules,
        report.line_count as u64
    );
    assert!(report.qzi_ngram_bytes > 0);
    assert!(report.qzi_ngram_size_ratio > 0.0);
    assert_eq!(report.rare_token_query.verified_matches, 1);
    assert_eq!(report.rare_token_query.iterations, report.query_repetitions);
    assert_eq!(
        report.rare_token_query.warmup_iterations,
        report.query_warmup_repetitions
    );
    assert!(report.query_repetitions > 0);
    assert!(report.query_warmup_repetitions > 0);
    assert!(report.pack_mib_s > 0.0);
    assert!(report.export_mib_s > 0.0);
    assert!(report.range_mib_s > 0.0);
}

#[derive(Debug, Clone, Copy)]
enum MatrixCorpusKind {
    Ascii,
    Utf8Mixed,
    Japanese,
}

impl MatrixCorpusKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Ascii => "ascii",
            Self::Utf8Mixed => "utf8-mixed",
            Self::Japanese => "japanese",
        }
    }
}

fn build_profile_corpus(target_bytes: usize, kind: MatrixCorpusKind) -> (Vec<u8>, usize) {
    let mut corpus = Vec::with_capacity(target_bytes);
    let mut line = 0usize;

    while corpus.len() < target_bytes {
        let line_text = if line == 0 {
            match kind {
                MatrixCorpusKind::Ascii => format!(
                    "aaa ts={line:07} level=error service=qzt rare-token-unique message=needle line={line}",
                )
                .into_bytes(),
                MatrixCorpusKind::Utf8Mixed => format!(
                    "aaa ts={line:07} レベル=info サービス=qzt rare-token-unique message=needle utf8=µ{line}",
                )
                .into_bytes(),
                MatrixCorpusKind::Japanese => format!(
                    "aaa ts={line:07} レベル=error サービス=qzt rare-token-unique message=稀有記号 line={line}",
                )
                .into_bytes(),
            }
        } else {
            match kind {
                MatrixCorpusKind::Ascii => format!(
                    "aaa ts={line:07} level=info service=qzt component=release message=repeated benchmark corpus line={line}",
                )
                .into_bytes(),
                MatrixCorpusKind::Utf8Mixed => format!(
                    "aaa ts={line:07} level=info サービス=qzt component=release message=ベンチマーク unicode混在 line={line}",
                )
                .into_bytes(),
                MatrixCorpusKind::Japanese => format!(
                    "aaa ts={line:07} レベル=情報 サービス=qzt component=release message=ベンチマーク line={line}",
                )
                .into_bytes(),
            }
        };

        corpus.extend_from_slice(&line_text);
        corpus.push(b'\n');
        line += 1;
    }

    (corpus, line)
}

fn env_usize(name: &str, default: usize) -> usize {
    let Ok(raw) = std::env::var(name) else {
        return default;
    };

    let parsed = raw
        .parse::<usize>()
        .unwrap_or_else(|_| panic!("{name} must be a positive integer, got {raw:?}"));

    assert!(parsed > 0, "{name} must be greater than 0, got {raw:?}");

    parsed
}
