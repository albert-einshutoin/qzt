use qzt::benchmark::{run_competitive_benchmark, CompetitiveBenchmarkOptions};
use qzt::corpus::CorpusKind;

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
}
