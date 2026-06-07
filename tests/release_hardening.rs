use qzt::benchmark::{run_release_benchmark, ReleaseBenchmarkOptions};

#[test]
fn release_benchmark_reports_reproducible_large_corpus_metrics() {
    let report = run_release_benchmark(ReleaseBenchmarkOptions {
        line_count: 24_000,
        ..ReleaseBenchmarkOptions::default()
    })
    .expect("release benchmark should run");

    assert!(report.corpus_bytes >= 1_000_000);
    assert_eq!(report.exported_bytes, report.corpus_bytes);
    assert_eq!(report.rare_token_verified_matches, 1);
    assert!(report.rare_token_decoded_bytes < report.raw_scan_decoded_bytes);
    assert_eq!(report.common_ngram_decoded_bytes, 0);
    assert!(report.common_ngram_capped);
    assert!(report.qzi_ngram_bytes > 0);
    assert!(report.qzi_ngram_size_ratio > 0.0);
    assert!(report.pack_mib_s > 0.0);
    assert!(report.export_mib_s > 0.0);
    assert!(report.range_mib_s > 0.0);

    eprintln!("{report}");
}
