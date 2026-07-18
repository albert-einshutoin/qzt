use crate::chunk_table::STARTS_WITH_LINE_CONTINUATION;
use crate::error::{QztError, Result};
use crate::primitives::usize_to_u64;

/// Writer options required by deterministic chunk planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkerOptions {
    /// Preferred uncompressed chunk size in bytes.
    pub target_chunk_size: usize,
    /// Hard maximum uncompressed chunk size in bytes.
    pub max_chunk_size: usize,
}

impl ChunkerOptions {
    /// Validates that both sizes are non-zero and the target does not exceed the maximum.
    ///
    /// # Errors
    ///
    /// Returns [`QztError::ResourceLimitExceeded`] when either size is zero or
    /// `target_chunk_size` is greater than `max_chunk_size`.
    pub fn validate(self) -> Result<()> {
        if self.target_chunk_size == 0 || self.max_chunk_size == 0 {
            return Err(QztError::ResourceLimitExceeded);
        }
        if self.target_chunk_size > self.max_chunk_size {
            return Err(QztError::ResourceLimitExceeded);
        }
        Ok(())
    }
}

/// Metadata derived for the whole original byte stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkPlan {
    pub chunks: Vec<PlannedChunk>,
    pub line_count: u64,
    pub newline_mode: NewlineMode,
}

/// Planned uncompressed chunk metadata before zstd writing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedChunk {
    pub chunk_id: u64,
    pub logical_offset: u64,
    pub uncompressed_size: u64,
    pub first_line: u64,
    pub line_count: u64,
    pub flags: u32,
}

/// Newline mode derived from original bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewlineMode {
    None,
    Lf,
    Crlf,
    Mixed,
}

impl NewlineMode {
    pub(crate) fn from_counts(lf_count: u64, crlf_count: u64) -> Self {
        match (lf_count > 0, crlf_count > 0) {
            (false, false) => Self::None,
            (true, false) => Self::Lf,
            (false, true) => Self::Crlf,
            (true, true) => Self::Mixed,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Lf => "lf",
            Self::Crlf => "crlf",
            Self::Mixed => "mixed",
        }
    }
}

/// Plans UTF-8-safe, CRLF-safe chunks without performing compression.
pub fn plan_chunks(input: &[u8], options: ChunkerOptions) -> Result<ChunkPlan> {
    options.validate()?;
    std::str::from_utf8(input).map_err(|_| QztError::InvalidUtf8)?;

    let line_info = LineInfo::from_bytes(input);
    if input.is_empty() {
        return Ok(ChunkPlan {
            chunks: Vec::new(),
            line_count: 0,
            newline_mode: line_info.newline_mode,
        });
    }

    let mut chunks = Vec::new();
    let mut start = 0_usize;

    while start < input.len() {
        let end = choose_chunk_end(input, start, options)?;
        if end <= start {
            return Err(QztError::ResourceLimitExceeded);
        }

        let first_line = lower_bound(&line_info.line_starts, start);
        let line_end = lower_bound(&line_info.line_starts, end);
        let flags = if start > 0 && input[start - 1] != b'\n' {
            STARTS_WITH_LINE_CONTINUATION
        } else {
            0
        };

        chunks.push(PlannedChunk {
            chunk_id: usize_to_u64(chunks.len())?,
            logical_offset: usize_to_u64(start)?,
            uncompressed_size: usize_to_u64(end - start)?,
            first_line: usize_to_u64(first_line)?,
            line_count: usize_to_u64(line_end - first_line)?,
            flags,
        });

        start = end;
    }

    Ok(ChunkPlan {
        chunks,
        line_count: usize_to_u64(line_info.line_starts.len())?,
        newline_mode: line_info.newline_mode,
    })
}

fn choose_chunk_end(
    input: &[u8],
    start: usize,
    options: ChunkerOptions,
) -> Result<usize> {
    let remaining = input.len() - start;
    // Use target_chunk_size as the soft limit: pack all remaining into one
    // chunk only when it fits within the target, not when it fits within max.
    // This keeps chunk sizes close to target even for the final window of a
    // large file, which matters for memory-profile containers where chunk size
    // controls retrieval granularity.
    if remaining <= options.target_chunk_size {
        return Ok(input.len());
    }

    let target_end = start + options.target_chunk_size;
    // Clamp max_end to input length: after the target-size guard above, the
    // remaining bytes may be less than max_chunk_size, so start+max could
    // exceed the slice boundary.
    let max_end = (start + options.max_chunk_size).min(input.len());

    if let Some(line_end) = last_line_boundary(input, start, target_end) {
        return Ok(line_end);
    }

    if let Some(line_end) = last_line_boundary(input, start, max_end) {
        return Ok(line_end);
    }

    previous_valid_split(input, start, max_end).ok_or(QztError::ResourceLimitExceeded)
}

pub(crate) fn last_line_boundary(input: &[u8], start: usize, end: usize) -> Option<usize> {
    let mut cursor = start;
    let mut boundary = None;
    while cursor < end {
        if input[cursor] == b'\n' {
            boundary = Some(cursor + 1);
        }
        cursor += 1;
    }
    boundary.filter(|candidate| *candidate > start && !splits_crlf(input, *candidate))
}

pub(crate) fn previous_valid_split(input: &[u8], start: usize, max_end: usize) -> Option<usize> {
    (start + 1..=max_end)
        .rev()
        .find(|candidate| is_utf8_boundary(input, *candidate) && !splits_crlf(input, *candidate))
}

pub(crate) fn splits_crlf(input: &[u8], end: usize) -> bool {
    end > 0 && end < input.len() && input[end - 1] == b'\r' && input[end] == b'\n'
}

pub(crate) fn is_utf8_boundary(input: &[u8], index: usize) -> bool {
    index == 0
        || index == input.len()
        || input
            .get(index)
            .is_some_and(|byte| byte & 0b1100_0000 != 0b1000_0000)
}

fn lower_bound(values: &[usize], target: usize) -> usize {
    values.partition_point(|value| *value < target)
}

struct LineInfo {
    line_starts: Vec<usize>,
    newline_mode: NewlineMode,
}

impl LineInfo {
    fn from_bytes(input: &[u8]) -> Self {
        if input.is_empty() {
            return Self {
                line_starts: Vec::new(),
                newline_mode: NewlineMode::None,
            };
        }

        let mut line_starts = vec![0];
        let mut lf_count = 0_u64;
        let mut crlf_count = 0_u64;

        for index in 0..input.len() {
            if input[index] != b'\n' {
                continue;
            }

            if index > 0 && input[index - 1] == b'\r' {
                crlf_count += 1;
            } else {
                lf_count += 1;
            }

            if index + 1 < input.len() {
                line_starts.push(index + 1);
            }
        }

        Self {
            line_starts,
            newline_mode: NewlineMode::from_counts(lf_count, crlf_count),
        }
    }
}
