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
fn release_value_targets_hold_for_ten_megabyte_logs_with_explicit_build_admission() {
    let (corpus, line_count) = build_profile_corpus(10_000_000, MatrixCorpusKind::Ascii);
    let report = run_release_benchmark_with_corpus(
        &corpus,
        ReleaseBenchmarkOptions {
            line_count,
            query_repetitions: 1,
            query_warmup_repetitions: 0,
            // This retained value benchmark measures admitted success. The
            // narrower product-default refusal is covered by the budget tests.
            index_build_limits: qzt::IndexBuildLimits {
                max_posting_ids: 16_000_000,
                ..Default::default()
            },
            ..ReleaseBenchmarkOptions::default()
        },
    )
    .expect("10 MB value benchmark with explicit build admission should run");
    eprintln!("10 MB value benchmark: max_build_postings=16000000; other build limits=default");

    assert!(
        report.compression_ratio <= 0.15,
        "log container must stay within the documented C2 target: {report}"
    );
    assert!(
        report.qzi_token_size_ratio <= 1.70,
        "token sidecar must stay within the documented size target: {report}"
    );
    assert!(
        report.qzi_ngram_size_ratio <= 1.70,
        "ngram sidecar must stay within the documented size target: {report}"
    );
    assert!(
        report.rare_token_decoded_bytes.saturating_mul(100) < report.corpus_bytes,
        "rare-token search must decode less than 1% of the source: {report}"
    );
}

#[test]
fn release_benchmark_applies_the_supplied_build_budget() {
    assert_eq!(
        run_release_benchmark_with_corpus(
            b"aaa\n",
            ReleaseBenchmarkOptions {
                line_count: 1,
                range_size: 1,
                index_build_limits: qzt::IndexBuildLimits {
                    max_granules: 0,
                    ..Default::default()
                },
                ..Default::default()
            }
        ),
        Err(qzt::QztError::ResourceLimitExceeded)
    );
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
    let defaults = qzt::IndexBuildLimits::default();
    let build_limits = qzt::IndexBuildLimits {
        max_granules: env_budget(
            "QZT_RELEASE_BENCH_MAX_BUILD_GRANULES",
            defaults.max_granules,
        ),
        max_distinct_keys: env_budget(
            "QZT_RELEASE_BENCH_MAX_BUILD_KEYS",
            defaults.max_distinct_keys,
        ),
        max_posting_ids: env_budget(
            "QZT_RELEASE_BENCH_MAX_BUILD_POSTINGS",
            defaults.max_posting_ids,
        ),
        max_key_bytes: env_budget(
            "QZT_RELEASE_BENCH_MAX_BUILD_KEY_BYTES",
            defaults.max_key_bytes,
        ),
        max_encoded_bytes: env_budget(
            "QZT_RELEASE_BENCH_MAX_BUILD_ENCODED_BYTES",
            defaults.max_encoded_bytes,
        ),
    };
    eprintln!("[profile-matrix] explicit_build_limits={build_limits:?}");

    for (corpus_label, corpus_bytes) in CORPUS_SIZES {
        for kind in CORPUS_KINDS {
            let (corpus, line_count) = build_profile_corpus(corpus_bytes, kind);
            let outcome = run_release_benchmark_with_corpus(
                &corpus,
                ReleaseBenchmarkOptions {
                    line_count,
                    query_repetitions,
                    query_warmup_repetitions,
                    index_build_limits: build_limits,
                    ..ReleaseBenchmarkOptions::default()
                },
            );
            let Some(report) = profile_outcome(outcome) else {
                eprintln!(
                    "[profile-matrix] corpus={corpus_label} kind={} status=resource_limit_exceeded measured_success=false",
                    kind.label()
                );
                continue;
            };

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

fn profile_outcome(
    outcome: Result<ReleaseBenchmarkReport, qzt::QztError>,
) -> Option<ReleaseBenchmarkReport> {
    match outcome {
        Ok(report) => Some(report),
        Err(qzt::QztError::ResourceLimitExceeded) => None,
        Err(error) => panic!("profile failed: {error}"),
    }
}

fn env_budget(name: &str, default: u64) -> u64 {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .unwrap_or_else(|_| panic!("{name} must be a nonnegative u64")),
        Err(std::env::VarError::NotPresent) => default,
        Err(error) => panic!("invalid {name}: {error}"),
    }
}

#[test]
fn matrix_refusal_is_not_a_success_and_does_not_hide_a_following_case() {
    let denied = ReleaseBenchmarkOptions {
        line_count: 1,
        range_size: 1,
        query_repetitions: 1,
        query_warmup_repetitions: 0,
        index_build_limits: qzt::IndexBuildLimits {
            max_granules: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let outcomes: Vec<_> = [
        denied,
        ReleaseBenchmarkOptions {
            index_build_limits: qzt::IndexBuildLimits::default(),
            ..denied
        },
    ]
    .into_iter()
    .map(|options| profile_outcome(run_release_benchmark_with_corpus(b"aaa\n", options)))
    .collect();
    assert!(outcomes[0].is_none());
    assert_eq!(outcomes[1].as_ref().unwrap().exported_bytes, 4);
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
