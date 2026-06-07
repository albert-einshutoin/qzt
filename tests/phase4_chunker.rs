use qzt::chunk_table::STARTS_WITH_LINE_CONTINUATION;
use qzt::chunker::{plan_chunks, ChunkerOptions, NewlineMode};
use qzt::error::QztError;

fn options(target_chunk_size: usize, max_chunk_size: usize) -> ChunkerOptions {
    ChunkerOptions {
        target_chunk_size,
        max_chunk_size,
    }
}

#[test]
fn invalid_utf8_is_rejected_before_chunk_planning() {
    assert_eq!(
        plan_chunks(&[0xff], options(4, 8)).map(|plan| plan.chunks),
        Err(QztError::InvalidUtf8)
    );
}

#[test]
fn empty_input_produces_no_chunks_and_zero_lines() {
    let plan = plan_chunks(b"", options(4, 8)).expect("empty input should plan");

    assert!(plan.chunks.is_empty());
    assert_eq!(plan.line_count, 0);
    assert_eq!(plan.newline_mode, NewlineMode::None);
}

#[test]
fn ascii_input_plans_contiguous_logical_offsets_and_lines() {
    let plan = plan_chunks(b"abc\ndef", options(3, 4)).expect("ASCII should plan");

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
    let plan = plan_chunks(input, options(4, 5)).expect("UTF-8 should plan");

    assert_eq!(plan.chunks.len(), 3);
    for chunk in &plan.chunks {
        let start = chunk.logical_offset as usize;
        let end = start + chunk.uncompressed_size as usize;
        assert!(std::str::from_utf8(&input[start..end]).is_ok());
    }
}

#[test]
fn crlf_boundary_is_not_split_between_cr_and_lf() {
    let input = b"a\r\nb";
    let plan = plan_chunks(input, options(2, 2)).expect("CRLF input should plan");

    for chunk in &plan.chunks {
        let end = chunk.logical_offset as usize + chunk.uncompressed_size as usize;
        assert!(!(end > 0 && end < input.len() && input[end - 1] == b'\r' && input[end] == b'\n'));
    }
    assert_eq!(plan.newline_mode, NewlineMode::Crlf);
    assert_eq!(plan.line_count, 2);
}

#[test]
fn long_line_exceeding_max_chunk_size_is_split_safely() {
    let plan = plan_chunks(b"abcdef", options(2, 3)).expect("long line should split");

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
        plan_chunks(input, options(1, 3)).map(|plan| plan.chunks),
        Err(QztError::ResourceLimitExceeded)
    );
}

#[test]
fn chunk_line_counts_sum_to_container_line_count_and_first_lines_are_contiguous() {
    let plan = plan_chunks(b"a\nbc\ndef", options(2, 4)).expect("input should plan");

    let sum: u64 = plan.chunks.iter().map(|chunk| chunk.line_count).sum();
    assert_eq!(sum, plan.line_count);

    for pair in plan.chunks.windows(2) {
        assert_eq!(pair[1].first_line, pair[0].first_line + pair[0].line_count);
    }
}
