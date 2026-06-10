use qzt::search::SearchReport;

pub(crate) fn assert_semantic_report_eq(
    left: &SearchReport,
    right: &SearchReport,
    label: &str,
) {
    assert_eq!(left.hits, right.hits, "hits mismatch: {label}");
    assert_eq!(left.capped, right.capped, "capped mismatch: {label}");
    assert_eq!(
        left.metrics.term_lookups,
        right.metrics.term_lookups,
        "term_lookups mismatch: {label}"
    );
    assert_eq!(
        left.metrics.verified_matches,
        right.metrics.verified_matches,
        "verified_matches mismatch: {label}"
    );
    assert_eq!(
        left.metrics.candidate_granules,
        right.metrics.candidate_granules,
        "candidate_granules mismatch: {label}"
    );
    assert_eq!(
        left.metrics.decoded_bytes,
        right.metrics.decoded_bytes,
        "decoded_bytes mismatch: {label}"
    );
    assert_eq!(
        left.metrics.physical_decoded_bytes,
        right.metrics.physical_decoded_bytes,
        "physical_decoded_bytes mismatch: {label}"
    );
    assert_eq!(
        left.metrics.candidate_chunks,
        right.metrics.candidate_chunks,
        "candidate_chunks mismatch: {label}"
    );
    assert_eq!(
        left.incomplete_reason, right.incomplete_reason,
        "incomplete_reason mismatch: {label}"
    );
    assert_eq!(
        left.planner.selected_keys,
        right.planner.selected_keys,
        "planner.selected_keys mismatch: {label}"
    );
    assert_eq!(
        left.planner.missing_keys,
        right.planner.missing_keys,
        "planner.missing_keys mismatch: {label}"
    );
    assert_eq!(
        left.planner.high_df_keys,
        right.planner.high_df_keys,
        "planner.high_df_keys mismatch: {label}"
    );
}
