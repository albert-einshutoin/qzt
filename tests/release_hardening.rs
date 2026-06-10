use qzt::benchmark::{run_release_benchmark, ReleaseBenchmarkOptions, ReleaseBenchmarkReport};

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

fn env_usize(name: &str, default: usize) -> usize {
    let raw = match std::env::var(name) {
        Ok(raw) => raw,
        Err(_) => return default,
    };

    let parsed = raw
        .parse::<usize>()
        .unwrap_or_else(|_| panic!("{name} must be a positive integer, got {raw:?}"));

    if parsed == 0 {
        panic!("{name} must be greater than 0, got {raw:?}");
    }

    parsed
}
