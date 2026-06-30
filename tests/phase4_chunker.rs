use qzt::chunk_table::STARTS_WITH_LINE_CONTINUATION;
use qzt::chunker::{NewlineMode, plan_chunks};
use qzt::error::QztError;
mod support;

#[test]
fn invalid_utf8_is_rejected_before_chunk_planning() {
    assert_eq!(
        plan_chunks(&[0xff], support::chunker_options(4, 8)).map(|plan| plan.chunks),
        Err(QztError::InvalidUtf8)
    );
}

#[test]
fn empty_input_produces_no_chunks_and_zero_lines() {
    let plan = plan_chunks(b"", support::chunker_options(4, 8)).expect("empty input should plan");

    assert!(plan.chunks.is_empty());
    assert_eq!(plan.line_count, 0);
    assert_eq!(plan.newline_mode, NewlineMode::None);
}

#[test]
fn ascii_input_plans_contiguous_logical_offsets_and_lines() {
    let plan = plan_chunks(b"abc\ndef", support::chunker_options(3, 4)).expect("ASCII should plan");

    assert_eq!(plan.line_count, 2);
    assert_eq!(plan.newline_mode, NewlineMode::Lf);
    assert_eq!(plan.chunks.len(), 2);
    assert_eq!(plan.chunks[0].logical_offset, 0);
    assert_eq!(plan.chunks[0].uncompressed_size, 4);
    assert_eq!(plan.chunks[0].first_line, 0);
    assert_eq!(plan.chunks[0].line_count, 1);
    assert_eq!(plan.chunks[1].logical_offset, 4);
    assert_eq!(plan.chunks[1].uncompressed_size, 3);
    assert_eq!(plan.chunks[1].first_line, 1);
    assert_eq!(plan.chunks[1].line_count, 1);
}

#[test]
fn japanese_and_emoji_boundaries_are_never_split() {
    let input = "あ😀い".as_bytes();
    let plan = plan_chunks(input, support::chunker_options(4, 5)).expect("UTF-8 should plan");

    assert_eq!(plan.chunks.len(), 3);
    for chunk in &plan.chunks {
        let start = usize::try_from(chunk.logical_offset).expect("fits in tests");
        let end = start + usize::try_from(chunk.uncompressed_size).expect("fits in tests");
        assert!(std::str::from_utf8(&input[start..end]).is_ok());
    }
}

#[test]
fn crlf_boundary_is_not_split_between_cr_and_lf() {
    let input = b"a\r\nb";
    let plan = plan_chunks(input, support::chunker_options(2, 2)).expect("CRLF input should plan");

    for chunk in &plan.chunks {
        let end = usize::try_from(chunk.logical_offset).expect("fits")
            + usize::try_from(chunk.uncompressed_size).expect("fits");
        assert!(!(end > 0 && end < input.len() && input[end - 1] == b'\r' && input[end] == b'\n'));
    }
    assert_eq!(plan.newline_mode, NewlineMode::Crlf);
    assert_eq!(plan.line_count, 2);
}

#[test]
fn long_line_exceeding_max_chunk_size_is_split_safely() {
    let plan =
        plan_chunks(b"abcdef", support::chunker_options(2, 3)).expect("long line should split");

    assert_eq!(plan.line_count, 1);
    assert_eq!(plan.chunks.len(), 2);
    assert_eq!(plan.chunks[0].uncompressed_size, 3);
    assert_eq!(plan.chunks[0].line_count, 1);
    assert_eq!(plan.chunks[1].uncompressed_size, 3);
    assert_eq!(plan.chunks[1].first_line, 1);
    assert_eq!(plan.chunks[1].line_count, 0);
    assert_eq!(plan.chunks[1].flags, STARTS_WITH_LINE_CONTINUATION);
}

#[test]
fn no_valid_utf8_boundary_within_max_returns_resource_limit() {
    let input = "😀".as_bytes();

    assert_eq!(
        plan_chunks(input, support::chunker_options(1, 3)).map(|plan| plan.chunks),
        Err(QztError::ResourceLimitExceeded)
    );
}

#[test]
fn remaining_between_target_and_max_produces_multiple_chunks() {
    // With old code: if remaining <= max_chunk_size, all remaining = 1 chunk.
    // With fix: only pack all remaining when remaining <= target_chunk_size.
    //
    // 300 bytes of "x\n" with target=100, max=1000:
    //   old → 300 <= 1000 → 1 chunk of 300 bytes
    //   new → 300 > 100 → split at target boundary → at least 3 chunks
    let input: Vec<u8> = b"x\n".iter().cycle().take(300).copied().collect();
    let plan = plan_chunks(&input, support::chunker_options(100, 1000)).expect("input should plan");

    assert!(
        plan.chunks.len() > 1,
        "remaining=300 > target=100 should produce multiple chunks, got {}",
        plan.chunks.len()
    );
    for chunk in &plan.chunks {
        assert!(
            usize::try_from(chunk.uncompressed_size).expect("fits") <= 1000,
            "chunk {} exceeds max_chunk_size",
            chunk.chunk_id
        );
    }
}

#[test]
fn chunk_line_counts_sum_to_container_line_count_and_first_lines_are_contiguous() {
    let plan =
        plan_chunks(b"a\nbc\ndef", support::chunker_options(2, 4)).expect("input should plan");

    let sum: u64 = plan.chunks.iter().map(|chunk| chunk.line_count).sum();
    assert_eq!(sum, plan.line_count);

    for pair in plan.chunks.windows(2) {
        assert_eq!(pair[1].first_line, pair[0].first_line + pair[0].line_count);
    }
}
