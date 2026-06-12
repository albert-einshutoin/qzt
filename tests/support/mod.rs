use qzt::search::SearchReport;

/// Compare semantically-equivalent search behavior between two execution paths.
///
/// Intentionally excluded fields:
/// - `metrics.query_time_ms`: runtime dependent and non-deterministic
/// - `metrics.index_size_bytes`: defined differently by source of metric
///   (`Raw` sidecar reports estimated in-memory bytes, while file sidecar
///   reports serialized section payload bytes; on skip-heavy indexes this can
///   make file-sidecar bytes smaller than in-memory estimates)
/// - `metrics.posting_bytes_read`: differs when skip-data is simulated
/// - `planner.used_skip_data`: file sidecar keeps it false even when the
///   in-memory index would use skip data
/// - `metrics.candidate_chunks`:
///   when capped=false, in-memory and file sidecar both report counts
///   consistently; when capped=true, file sidecar returns 0 early before counting
///   candidate chunks (see sidecar.rs), so this field is compared only when uncapped.
pub fn assert_semantic_report_eq(left: &SearchReport, right: &SearchReport, label: &str) {
    assert_eq!(left.hits, right.hits, "hits mismatch: {label}");
    assert_eq!(left.capped, right.capped, "capped mismatch: {label}");
    assert_eq!(
        left.metrics.term_lookups, right.metrics.term_lookups,
        "term_lookups mismatch: {label}"
    );
    assert_eq!(
        left.metrics.verified_matches, right.metrics.verified_matches,
        "verified_matches mismatch: {label}"
    );
    assert_eq!(
        left.metrics.candidate_granules, right.metrics.candidate_granules,
        "candidate_granules mismatch: {label}"
    );
    assert_eq!(
        left.metrics.decoded_bytes, right.metrics.decoded_bytes,
        "decoded_bytes mismatch: {label}"
    );
    assert_eq!(
        left.metrics.physical_decoded_bytes, right.metrics.physical_decoded_bytes,
        "physical_decoded_bytes mismatch: {label}"
    );
    if !left.capped {
        assert_eq!(
            left.metrics.candidate_chunks, right.metrics.candidate_chunks,
            "candidate_chunks mismatch: {label}"
        );
    }
    assert_eq!(
        left.incomplete_reason, right.incomplete_reason,
        "incomplete_reason mismatch: {label}"
    );
    assert_eq!(
        left.planner.selected_keys, right.planner.selected_keys,
        "planner.selected_keys mismatch: {label}"
    );
    assert_eq!(
        left.planner.missing_keys, right.planner.missing_keys,
        "planner.missing_keys mismatch: {label}"
    );
    assert_eq!(
        left.planner.high_df_keys, right.planner.high_df_keys,
        "planner.high_df_keys mismatch: {label}"
    );
}
