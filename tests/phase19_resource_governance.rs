use qzt::cbor::{CborLimits, CborValue, encode_deterministic, validate_deterministic_with_limits};
use qzt::error::QztError;
use qzt::limits::ResourceLimits;
use qzt::reader::QztReader;
use qzt::search::{RawTokenIndex, SearchOptions, TokenIndexBuildOptions};
use qzt::writer::pack_bytes;
mod support;
use support::writer_options;

#[test]
fn cbor_decoder_uses_caller_supplied_allocation_budget() {
    let encoded =
        encode_deterministic(&CborValue::Bytes(vec![0_u8; 16])).expect("cbor should encode");

    assert_eq!(
        validate_deterministic_with_limits(
            &encoded,
            CborLimits {
                max_allocation: 8,
                max_items: 1024,
            },
        ),
        Err(QztError::ResourceLimitExceeded)
    );
    assert!(
        validate_deterministic_with_limits(
            &encoded,
            CborLimits {
                max_allocation: 16,
                max_items: 1024,
            },
        )
        .is_ok()
    );
}

#[test]
fn cbor_decoder_rejects_excessive_nesting_before_recursing_unboundedly() {
    let mut encoded = vec![0x81; 65];
    encoded.push(0xf6);

    assert_eq!(
        validate_deterministic_with_limits(&encoded, CborLimits::default()),
        Err(QztError::ResourceLimitExceeded)
    );
}

#[test]
fn cbor_decoder_applies_allocation_budget_to_the_complete_value() {
    let encoded = encode_deterministic(&CborValue::Array(vec![
        CborValue::Bytes(vec![0_u8; 8]),
        CborValue::Bytes(vec![1_u8; 8]),
    ]))
    .expect("cbor should encode");

    assert_eq!(
        validate_deterministic_with_limits(
            &encoded,
            CborLimits {
                max_allocation: 8,
                max_items: 1024,
            },
        ),
        Err(QztError::ResourceLimitExceeded)
    );
}

#[test]
fn cbor_decoder_applies_item_budget_across_nested_containers() {
    let encoded = encode_deterministic(&CborValue::Array(vec![
        CborValue::Array(vec![CborValue::Null, CborValue::Null]),
        CborValue::Array(vec![CborValue::Null, CborValue::Null]),
    ]))
    .expect("cbor should encode");

    assert_eq!(
        validate_deterministic_with_limits(
            &encoded,
            CborLimits {
                max_allocation: 1024,
                max_items: 2,
            },
        ),
        Err(QztError::ResourceLimitExceeded)
    );
}

#[test]
fn open_with_limits_threads_cbor_budget_into_metadata_decode() {
    let input = b"alpha\nbeta\n";
    let container = pack_bytes(input, writer_options(64, 64)).expect("pack");
    let limits = ResourceLimits {
        max_cbor_allocation: 4,
        ..ResourceLimits::default()
    };

    assert_eq!(
        QztReader::open_with_limits(&container, limits).map(|_| ()),
        Err(QztError::ResourceLimitExceeded)
    );
}

#[test]
fn search_result_cap_limits_hits_and_marks_report_capped() {
    let input = b"needle one\nneedle two\nneedle three\n";
    let container = pack_bytes(input, writer_options(64, 64)).expect("pack");
    let reader = QztReader::open(&container).expect("reader");
    let index = RawTokenIndex::build_from_container(&container, TokenIndexBuildOptions::default())
        .expect("token index");

    let report = index
        .search(
            &reader,
            "needle",
            SearchOptions {
                max_candidate_granules: 100,
                max_decoded_bytes: 1024 * 1024,
                max_search_results: 2,
            },
        )
        .expect("search");

    assert!(report.capped);
    assert_eq!(report.hits.len(), 2);
    assert_eq!(report.metrics.verified_matches, 2);
}
