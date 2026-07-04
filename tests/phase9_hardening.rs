use std::panic;

use qzt::chunker::ChunkerOptions;
use qzt::reader::{QztFileReader, QztReader, VerifyLevel};
use qzt::skeleton::open_skeleton_details;
use qzt::writer::{WriterOptions, pack_bytes_with_container_id};
mod support;
use support::writer_options;

const CORE_CONFORMANCE_MAP: &[(u8, &str, &str)] = &[
    (
        1,
        "empty file",
        "phase5_writer::empty_file_pack_export_equality",
    ),
    (
        2,
        "one line without newline",
        "phase7_access::read_line_raw_reads_first_last_and_spanning_lines",
    ),
    (
        3,
        "one line with newline",
        "phase9_cli_core::cli_pack_info_verify_range_lines_and_export_round_trip",
    ),
    (
        4,
        "LF multi-line",
        "phase6_reader::reader_open_info_and_export_work_for_phase5_container",
    ),
    (
        5,
        "CRLF multi-line",
        "phase5_writer::crlf_and_mixed_newline_pack_export_equality",
    ),
    (
        6,
        "mixed LF/CRLF",
        "phase5_writer::crlf_and_mixed_newline_pack_export_equality",
    ),
    (
        7,
        "lone CR ordinary data",
        "phase5_writer::crlf_and_mixed_newline_pack_export_equality",
    ),
    (
        8,
        "Japanese UTF-8",
        "phase5_writer::japanese_and_emoji_pack_export_equality",
    ),
    (
        9,
        "emoji UTF-8",
        "phase5_writer::japanese_and_emoji_pack_export_equality",
    ),
    (
        10,
        "invalid UTF-8 rejected",
        "phase9_cli_core::cli_pack_rejects_invalid_utf8",
    ),
    (
        11,
        "long line over target",
        "phase4_chunker::long_line_exceeding_max_chunk_size_is_split_safely",
    ),
    (
        12,
        "long line over max",
        "phase4_chunker::long_line_exceeding_max_chunk_size_is_split_safely",
    ),
    (
        13,
        "spanning read_line",
        "phase7_access::read_line_raw_reads_first_last_and_spanning_lines",
    ),
    (
        14,
        "Japanese boundary",
        "phase4_chunker::japanese_and_emoji_boundaries_are_never_split",
    ),
    (
        15,
        "CRLF boundary",
        "phase4_chunker::crlf_boundary_is_not_split_between_cr_and_lf",
    ),
    (
        16,
        "small chunk size",
        "phase7_access::read_range_within_one_chunk_and_across_chunks",
    ),
    (
        17,
        "no dictionary",
        "phase5_writer::ascii_file_pack_export_equality",
    ),
    (
        18,
        "embedded dictionary",
        "phase8_reader_core::dictionary_compressed_fixture_exports_exactly",
    ),
    (
        19,
        "missing dictionary",
        "phase8_reader_core::missing_dictionary_is_rejected_at_open",
    ),
    (
        20,
        "duplicate dictionary_id",
        "phase8_reader_core::duplicate_dictionary_id_is_rejected",
    ),
    (
        21,
        "dictionary checksum",
        "phase8_reader_core::dictionary_checksum_mismatch_is_rejected",
    ),
    (
        22,
        "Header magic",
        "phase2_fixed::header_rejects_invalid_magic_flags_reserved_and_version",
    ),
    (
        23,
        "Header reserved",
        "phase2_fixed::header_rejects_invalid_magic_flags_reserved_and_version",
    ),
    (
        24,
        "header flags",
        "phase2_fixed::header_rejects_invalid_magic_flags_reserved_and_version",
    ),
    (
        25,
        "unsupported version",
        "phase2_fixed::header_rejects_invalid_magic_flags_reserved_and_version",
    ),
    (
        26,
        "version mismatch",
        "phase2_fixed::footer_trailer_rejects_corrupt_magic_length_and_version",
    ),
    (
        27,
        "Footer Trailer",
        "phase2_fixed::footer_trailer_rejects_corrupt_magic_length_and_version",
    ),
    (
        28,
        "Footer checksum",
        "phase3_skeleton::footer_payload_checksum_mismatch_is_rejected",
    ),
    (
        29,
        "final_file_size",
        "phase3_skeleton::footer_payload_checksum_mismatch_is_rejected",
    ),
    (
        30,
        "container_id mismatch",
        "phase3_skeleton::header_footer_container_id_mismatch_is_rejected",
    ),
    (
        31,
        "metadata offset mismatch",
        "phase3_skeleton::header_footer_container_id_mismatch_is_rejected",
    ),
    (
        32,
        "Metadata checksum",
        "phase3_skeleton::metadata_and_index_root_source_mismatch_is_rejected",
    ),
    (
        33,
        "Metadata non-canonical",
        "phase1_cbor::rejects_non_shortest_integer_encoding",
    ),
    (
        34,
        "Metadata duplicate key",
        "phase1_cbor::rejects_duplicate_map_keys",
    ),
    (
        35,
        "Metadata container_id",
        "phase3_skeleton::metadata_and_index_root_source_mismatch_is_rejected",
    ),
    (
        36,
        "original_size mismatch",
        "phase3_skeleton::metadata_and_index_root_source_mismatch_is_rejected",
    ),
    (
        37,
        "original_checksum mismatch",
        "phase3_skeleton::metadata_and_index_root_source_mismatch_is_rejected",
    ),
    (
        38,
        "line_count mismatch",
        "phase3_skeleton::metadata_and_index_root_source_mismatch_is_rejected",
    ),
    (
        39,
        "Index Root checksum",
        "phase3_skeleton::metadata_and_index_root_source_mismatch_is_rejected",
    ),
    (
        40,
        "Index Root non-canonical",
        "phase1_cbor::rejects_unsorted_map_keys",
    ),
    (
        41,
        "block overlap",
        "phase2_fixed::physical_ranges_are_half_open_and_must_not_overlap_reserved_or_each_other",
    ),
    (
        42,
        "unknown optional block",
        "phase8_reader_core::unknown_optional_block_is_ignored_but_unknown_required_block_is_rejected",
    ),
    (
        43,
        "unknown required block",
        "phase8_reader_core::unknown_optional_block_is_ignored_but_unknown_required_block_is_rejected",
    ),
    (
        44,
        "block flags",
        "phase8_reader_core::unknown_optional_block_is_ignored_but_unknown_required_block_is_rejected",
    ),
    (
        45,
        "Chunk Table checksum",
        "phase3_skeleton::chunk_table_block_size_must_match_fixed_record_size",
    ),
    (
        46,
        "chunk_id sequence",
        "phase3_skeleton::chunk_count_mismatch_is_rejected",
    ),
    (
        47,
        "logical gap",
        "phase3_skeleton::chunk_count_mismatch_is_rejected",
    ),
    (
        48,
        "physical out of bounds",
        "phase2_fixed::physical_ranges_reject_too_small_files_and_out_of_bounds_ranges",
    ),
    (
        49,
        "physical overlap",
        "phase2_fixed::physical_ranges_are_half_open_and_must_not_overlap_reserved_or_each_other",
    ),
    (
        50,
        "first_line continuity",
        "phase4_chunker::chunk_line_counts_sum_to_container_line_count_and_first_lines_are_contiguous",
    ),
    (
        51,
        "sum line_count",
        "phase4_chunker::chunk_line_counts_sum_to_container_line_count_and_first_lines_are_contiguous",
    ),
    (
        52,
        "unknown chunk flag",
        "phase3_skeleton::chunk_table_block_size_must_match_fixed_record_size",
    ),
    (
        53,
        "continuation flag deep",
        "phase6_reader::deep_verify_decompresses_and_reports_decoded_bytes",
    ),
    (
        54,
        "compressed corruption",
        "phase6_reader::normal_verify_detects_compressed_chunk_checksum_mismatch",
    ),
    (
        55,
        "uncompressed checksum",
        "phase5_writer::compressed_and_uncompressed_checksums_match_exact_bytes",
    ),
    (
        56,
        "read_range one chunk",
        "phase7_access::read_range_within_one_chunk_and_across_chunks",
    ),
    (
        57,
        "read_range multi chunk",
        "phase7_access::read_range_within_one_chunk_and_across_chunks",
    ),
    (
        58,
        "range overflow",
        "phase7_access::read_range_zero_length_and_overflow_are_handled",
    ),
    (
        59,
        "text boundary",
        "phase7_access::read_text_range_rejects_invalid_utf8_boundary",
    ),
    (
        60,
        "first line",
        "phase7_access::read_line_raw_reads_first_last_and_spanning_lines",
    ),
    (
        61,
        "last line no newline",
        "phase7_access::read_line_raw_reads_first_last_and_spanning_lines",
    ),
    (
        62,
        "last line newline",
        "phase9_cli_core::cli_pack_info_verify_range_lines_and_export_round_trip",
    ),
    (
        63,
        "line out of range",
        "phase7_access::read_line_raw_reads_first_last_and_spanning_lines",
    ),
    (
        64,
        "Dense Line Index final line",
        "phase10_dense_line_index::dense_line_index_reads_final_line_without_newline",
    ),
    (
        65,
        "Dense Line Index count",
        "phase10_dense_line_index::dense_line_index_count_mismatch_is_rejected",
    ),
    (
        66,
        "export equality",
        "phase5_writer::ascii_file_pack_export_equality",
    ),
    (
        67,
        "quick verify",
        "phase6_reader::quick_verify_succeeds_without_decompressing_corrupt_compressed_chunk",
    ),
    (
        68,
        "normal verify",
        "phase6_reader::normal_verify_detects_compressed_chunk_checksum_mismatch",
    ),
    (
        69,
        "deep line_count",
        "phase6_reader::deep_verify_decompresses_and_reports_decoded_bytes",
    ),
    (
        70,
        "Chunk Table size",
        "phase3_skeleton::chunk_table_block_size_must_match_fixed_record_size",
    ),
    (
        71,
        "chunk_count mismatch",
        "phase3_skeleton::chunk_count_mismatch_is_rejected",
    ),
    (
        72,
        "zero chunk",
        "phase3_skeleton::chunk_count_mismatch_is_rejected",
    ),
    (
        73,
        "sum uncompressed",
        "phase3_skeleton::chunk_count_mismatch_is_rejected",
    ),
    (
        74,
        "newline_mode",
        "phase6_reader::deep_verify_decompresses_and_reports_decoded_bytes",
    ),
    (
        75,
        "zstd output limit",
        "phase8_reader_core::resource_limits_are_enforced_before_decode",
    ),
    (
        76,
        "index_hint ignored",
        "phase2_fixed::invalid_index_hint_offset_is_non_authoritative_for_header_decode",
    ),
    (
        77,
        "container_checksum",
        "phase6_reader::normal_verify_detects_container_checksum_mismatch_when_present",
    ),
];

#[test]
fn core_conformance_map_covers_all_items() {
    assert_eq!(CORE_CONFORMANCE_MAP.len(), 77);
    for (expected, (actual, _, evidence)) in (1_u8..=77).zip(CORE_CONFORMANCE_MAP.iter()) {
        assert_eq!(expected, *actual);
        assert!(!evidence.is_empty());
    }
}

#[test]
fn crlf_chunk_boundary_conformance_pack_verify_export_range_and_line() {
    // Spec §8 (JP) / Core Spec §12.1 (EN): chunk boundaries MUST NOT split between CR
    // and LF in a CRLF sequence. Tiny chunk sizes force `\r` to be a boundary candidate.
    let input = b"a\r\nb\r\nc";
    let container = pack_bytes_with_container_id(input, [0x65; 16], writer_options(2, 2))
        .expect("CRLF fixture should pack");

    let details = open_skeleton_details(&container).expect("container should open");
    for entry in &details.chunk_entries {
        let end = usize::try_from(entry.logical_offset).expect("fits")
            + usize::try_from(entry.uncompressed_size).expect("fits");
        assert!(
            !(end > 0 && end < input.len() && input[end - 1] == b'\r' && input[end] == b'\n'),
            "chunk logical end must not fall between CR and LF"
        );
    }

    let reader = QztReader::open(&container).expect("reader should open");
    reader
        .verify(VerifyLevel::Deep)
        .expect("deep verify should pass");
    assert_eq!(reader.export_all(), Ok(input.to_vec()));
    assert_eq!(reader.read_range(1, 2), Ok(b"\r\n".to_vec()));
    assert_eq!(reader.read_line_raw(0), Ok(b"a\r\n".to_vec()));

    let file = QztFileReader::open_read_at(&container[..], container.len() as u64)
        .expect("file reader should open");
    assert_eq!(file.read_line_raw(0), Ok(b"a\r\n".to_vec()));
}

#[test]
fn malformed_open_and_verify_fuzz_smoke_does_not_panic() {
    let valid = pack_bytes_with_container_id(
        b"alpha\nbeta\ngamma\n",
        [0x99; 16],
        WriterOptions {
            chunker: ChunkerOptions {
                target_chunk_size: 8,
                max_chunk_size: 8,
            },
            zstd_level: 0,
        },
    )
    .expect("valid seed should pack");

    let mut seeds = vec![
        Vec::new(),
        vec![0],
        vec![0xff; 256],
        valid[..valid.len() / 2].to_vec(),
        valid.clone(),
    ];

    for index in [0, 1, 8, 16, 24, 40, 64, valid.len().saturating_sub(1)] {
        if index < valid.len() {
            let mut mutated = valid.clone();
            mutated[index] ^= 0xff;
            seeds.push(mutated);
        }
    }

    let mut state = 0x1234_5678_u64;
    for _ in 0..64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        #[allow(clippy::cast_possible_truncation)] // truncation is intentional for test RNG
        let len = (state as usize % 512) + 1;
        let mut bytes = Vec::with_capacity(len);
        for _ in 0..len {
            state = state
                .wrapping_mul(2_862_933_555_777_941_757)
                .wrapping_add(3_037_000_493);
            #[allow(clippy::cast_possible_truncation)] // intentional truncation for test RNG
            bytes.push((state >> 32) as u8);
        }
        seeds.push(bytes);
    }

    for seed in seeds {
        let result = panic::catch_unwind(|| {
            if let Ok(reader) = QztReader::open(&seed) {
                let _ = reader.verify(VerifyLevel::Quick);
                let _ = reader.verify(VerifyLevel::Normal);
                let _ = reader.verify(VerifyLevel::Deep);
            }
        });
        assert!(result.is_ok());
    }
}
