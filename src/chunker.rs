use crate::chunk_table::STARTS_WITH_LINE_CONTINUATION;
use crate::error::{QztError, Result};

/// Writer options required by deterministic chunk planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkerOptions {
    pub target_chunk_size: usize,
    pub max_chunk_size: usize,
}

impl ChunkerOptions {
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

/// Plans UTF-8-safe, CRLF-safe chunks without performing compression.
pub fn plan_chunks(input: &[u8], options: ChunkerOptions) -> Result<ChunkPlan> {
    options.validate()?;
    let text = std::str::from_utf8(input).map_err(|_| QztError::InvalidUtf8)?;

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
        let end = choose_chunk_end(input, text, start, options)?;
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
            chunk_id: u64::try_from(chunks.len()).map_err(|_| QztError::ResourceLimitExceeded)?,
            logical_offset: u64::try_from(start).map_err(|_| QztError::ResourceLimitExceeded)?,
            uncompressed_size: u64::try_from(end - start)
                .map_err(|_| QztError::ResourceLimitExceeded)?,
            first_line: u64::try_from(first_line).map_err(|_| QztError::ResourceLimitExceeded)?,
            line_count: u64::try_from(line_end - first_line)
                .map_err(|_| QztError::ResourceLimitExceeded)?,
            flags,
        });

        start = end;
    }

    Ok(ChunkPlan {
        chunks,
        line_count: u64::try_from(line_info.line_starts.len())
            .map_err(|_| QztError::ResourceLimitExceeded)?,
        newline_mode: line_info.newline_mode,
    })
}

fn choose_chunk_end(
    input: &[u8],
    text: &str,
    start: usize,
    options: ChunkerOptions,
) -> Result<usize> {
    let remaining = input.len() - start;
    if remaining <= options.max_chunk_size {
        return Ok(input.len());
    }

    let target_end = start + options.target_chunk_size;
    let max_end = start + options.max_chunk_size;

    if let Some(line_end) = last_line_boundary(input, start, target_end) {
        return Ok(line_end);
    }

    if let Some(line_end) = last_line_boundary(input, start, max_end) {
        return Ok(line_end);
    }

    previous_valid_split(input, text, start, max_end).ok_or(QztError::ResourceLimitExceeded)
}

fn last_line_boundary(input: &[u8], start: usize, end: usize) -> Option<usize> {
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

fn previous_valid_split(input: &[u8], text: &str, start: usize, max_end: usize) -> Option<usize> {
    (start + 1..=max_end)
        .rev()
        .find(|candidate| text.is_char_boundary(*candidate) && !splits_crlf(input, *candidate))
}

fn splits_crlf(input: &[u8], end: usize) -> bool {
    end > 0 && end < input.len() && input[end - 1] == b'\r' && input[end] == b'\n'
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

        let newline_mode = match (lf_count > 0, crlf_count > 0) {
            (false, false) => NewlineMode::None,
            (true, false) => NewlineMode::Lf,
            (false, true) => NewlineMode::Crlf,
            (true, true) => NewlineMode::Mixed,
        };

        Self {
            line_starts,
            newline_mode,
        }
    }
}
