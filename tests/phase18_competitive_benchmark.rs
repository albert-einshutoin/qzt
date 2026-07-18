use qzt::benchmark::{CompetitiveBenchmarkOptions, run_competitive_benchmark};
use qzt::corpus::CorpusKind;

#[path = "support/benchmark_log.rs"]
mod benchmark_log;

#[test]
fn competitive_benchmark_records_qzt_vs_raw_zstd_range_metrics() {
    let report = run_competitive_benchmark(CompetitiveBenchmarkOptions {
        corpus_kind: CorpusKind::C2Logs,
        corpus_bytes: 64 * 1024,
        chunk_size: 4 * 1024,
        range_offset: 8 * 1024,
        range_size: 1024,
    })
    .expect("competitive benchmark should run");

    assert_eq!(report.corpus_id, "C2");
    assert!(report.qzt_bytes > 0);
    assert!(report.raw_zstd_bytes > 0);
    assert_eq!(report.qzt_range_bytes, 1024);
    assert!(report.qzt_range_decoded_chunks > 0);
    assert!(report.qzt_range_decoded_bytes < report.corpus_bytes);
    assert!(report.qzt_range_compressed_bytes < report.qzt_bytes);
    assert_eq!(report.raw_zstd_decoded_bytes, report.corpus_bytes);
    assert!(report.token_hit_count > 0);
    assert!(report.reference_hit_count >= report.token_hit_count);
    assert_eq!(
        report.external_search_tools_enabled,
        cfg!(feature = "bench-compete")
    );
    if let Some(count) = report.ripgrep_hit_count {
        assert_eq!(count, report.reference_hit_count);
    }
    if let Some(count) = report.sqlite_fts5_hit_count {
        assert_eq!(count, report.reference_hit_count);
    }

    // Keep a stable, machine-readable record in --nocapture logs so published
    // comparisons can be audited instead of relying on Rust's Debug format.
    let fields = [
        ("corpus_id", report.corpus_id.to_owned()),
        ("corpus_bytes", report.corpus_bytes.to_string()),
        ("qzt_bytes", report.qzt_bytes.to_string()),
        ("raw_zstd_bytes", report.raw_zstd_bytes.to_string()),
        ("qzt_range_bytes", report.qzt_range_bytes.to_string()),
        (
            "qzt_range_decoded_chunks",
            report.qzt_range_decoded_chunks.to_string(),
        ),
        (
            "qzt_range_decoded_bytes",
            report.qzt_range_decoded_bytes.to_string(),
        ),
        (
            "qzt_range_compressed_bytes",
            report.qzt_range_compressed_bytes.to_string(),
        ),
        (
            "raw_zstd_decoded_bytes",
            report.raw_zstd_decoded_bytes.to_string(),
        ),
        ("qzt_range_micros", report.qzt_range_micros.to_string()),
        (
            "raw_zstd_range_micros",
            report.raw_zstd_range_micros.to_string(),
        ),
        ("token_hit_count", report.token_hit_count.to_string()),
        (
            "reference_hit_count",
            report.reference_hit_count.to_string(),
        ),
        (
            "external_search_tools_enabled",
            report.external_search_tools_enabled.to_string(),
        ),
        (
            "ripgrep_hit_count",
            report
                .ripgrep_hit_count
                .map_or_else(|| "none".to_owned(), |count| count.to_string()),
        ),
        (
            "sqlite_fts5_hit_count",
            report
                .sqlite_fts5_hit_count
                .map_or_else(|| "none".to_owned(), |count| count.to_string()),
        ),
    ];
    eprintln!("{}", benchmark_log::format_record(&fields));
}
