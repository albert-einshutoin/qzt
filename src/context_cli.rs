//! Bounded, read-only mapping from one global search-hit span to source bytes.

use std::fmt::Write as _;
use std::process::ExitCode;

use qzt::{DocumentEntry, QztError, QztFileReader, ReadAt};

use super::{cli_json, command_failed, parse_byte_limit, write_stdout};

const DEFAULT_SCAN: u64 = 256 * 1024;
const DEFAULT_PHYSICAL: u64 = 16 * 1024 * 1024;
const DEFAULT_CHUNKS: u64 = 64;
const DEFAULT_DOCUMENTS: u64 = 100_000;
const DEFAULT_EXCERPT: u64 = 64 * 1024;
const DEFAULT_OUTPUT: u64 = 1024 * 1024;
const MAX_CANDIDATES: usize = 256;

struct Options {
    offset: Option<u64>,
    length: Option<u64>,
    before: usize,
    after: usize,
    scan: u64,
    physical: u64,
    chunks: u64,
    documents: u64,
    excerpt: u64,
    output: u64,
    json: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            offset: None,
            length: None,
            before: 2,
            after: 2,
            scan: DEFAULT_SCAN,
            physical: DEFAULT_PHYSICAL,
            chunks: DEFAULT_CHUNKS,
            documents: DEFAULT_DOCUMENTS,
            excerpt: DEFAULT_EXCERPT,
            output: DEFAULT_OUTPUT,
            json: false,
        }
    }
}

pub(super) fn run(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt context: missing file");
        return ExitCode::from(2);
    };
    let mut options = Options::default();
    while let Some(flag) = args.next() {
        let Some(value) = args.next() else {
            eprintln!("qzt context: missing value for {flag}");
            return ExitCode::from(2);
        };
        let parsed = match flag.as_str() {
            "--offset"
            | "--length"
            | "--max-scan-bytes"
            | "--max-physical-decoded-bytes"
            | "--max-documents"
            | "--max-excerpt-bytes"
            | "--max-output-bytes" => parse_byte_limit(&value),
            "--before" | "--after" | "--max-physical-decoded-chunks" => value.parse().ok(),
            "--format" => {
                if value == "json" || value == "text" {
                    options.json = value == "json";
                    continue;
                }
                None
            }
            _ => None,
        };
        let Some(number) = parsed else {
            eprintln!("qzt context: invalid {flag}");
            return ExitCode::from(2);
        };
        match flag.as_str() {
            "--offset" => options.offset = Some(number),
            "--length" => options.length = Some(number),
            "--before" | "--after" => {
                let Ok(lines) = usize::try_from(number) else {
                    eprintln!("qzt context: invalid {flag}");
                    return ExitCode::from(2);
                };
                if lines > 1000 {
                    eprintln!("qzt context: {flag} exceeds 1000 lines");
                    return ExitCode::from(2);
                }
                if flag == "--before" {
                    options.before = lines;
                } else {
                    options.after = lines;
                }
            }
            "--max-scan-bytes" => options.scan = number,
            "--max-physical-decoded-bytes" => options.physical = number,
            "--max-physical-decoded-chunks" => options.chunks = number,
            "--max-documents" => options.documents = number,
            "--max-excerpt-bytes" => options.excerpt = number,
            "--max-output-bytes" => options.output = number,
            _ => unreachable!(),
        }
    }
    let (Some(offset), Some(length)) = (options.offset, options.length) else {
        eprintln!("qzt context: --offset and --length are required");
        return ExitCode::from(2);
    };
    if length == 0 || offset.checked_add(length).is_none() {
        eprintln!("qzt context: hit must be a positive, non-overflowing byte range");
        return ExitCode::from(2);
    }
    match build(&path, &options) {
        Ok(output) => write_stdout(output.as_bytes()),
        Err(error) => command_failed("context", &error.into()),
    }
}

struct View<'a> {
    hit_start: u64,
    hit_end: u64,
    scope_start: u64,
    scope_end: u64,
    scope_kind: &'static str,
    status: &'static str,
    document: Option<&'a DocumentEntry>,
    candidates: Vec<&'a DocumentEntry>,
    excerpt_start: u64,
    bytes: Vec<u8>,
    before_requested: usize,
    before_returned: usize,
    before_stop: &'static str,
    after_requested: usize,
    after_returned: usize,
    after_stop: &'static str,
    leading_fragment: bool,
    trailing_fragment: bool,
}

fn build(path: &str, options: &Options) -> qzt::Result<String> {
    let reader = QztFileReader::open_path(path)?;
    let details = reader.skeleton_details();
    let hit_start = options.offset.ok_or(QztError::ContainerCorrupt)?;
    let hit_end = hit_start
        .checked_add(options.length.ok_or(QztError::ContainerCorrupt)?)
        .ok_or(QztError::LogicalRangeOutOfBounds)?;
    let original_size = details.summary.original_size;
    if hit_end > original_size {
        return Err(QztError::LogicalRangeOutOfBounds);
    }
    if hit_end - hit_start > options.scan || hit_end - hit_start > options.excerpt {
        return Err(QztError::ResourceLimitExceeded);
    }
    let mut intersecting = Vec::new();
    let mut containing = Vec::new();
    let status = if let Some(index) = &details.document_index {
        if u64::try_from(index.documents.len()).map_err(|_| QztError::ResourceLimitExceeded)?
            > options.documents
        {
            return Err(QztError::ResourceLimitExceeded);
        }
        for document in &index.documents {
            let end = document
                .logical_offset
                .checked_add(document.byte_length)
                .ok_or(QztError::ContainerCorrupt)?;
            if end > original_size {
                return Err(QztError::ContainerCorrupt);
            }
            if document.logical_offset < hit_end && hit_start < end {
                if intersecting.len() == MAX_CANDIDATES {
                    return Err(QztError::ResourceLimitExceeded);
                }
                intersecting.push(document);
                if document.logical_offset <= hit_start && hit_end <= end {
                    containing.push(document);
                }
            }
        }
        if containing.len() == 1 && intersecting.len() == 1 {
            "unique"
        } else if !containing.is_empty() {
            "ambiguous"
        } else if !intersecting.is_empty() {
            "cross_document"
        } else {
            "unmapped"
        }
    } else {
        "no_document_index"
    };
    let document = if status == "unique" {
        Some(containing[0])
    } else {
        None
    };
    let (scope_start, scope_end, scope_kind) = if let Some(doc) = document {
        (
            doc.logical_offset,
            doc.logical_offset + doc.byte_length,
            "document",
        )
    } else {
        (0, original_size, "container")
    };

    // Exactly one range read follows. Its physical chunk cost is checked first;
    // neither a second line lookup nor a cache-dependent repeat decode occurs.
    let side_budget = (options.scan - (hit_end - hit_start)) / 2;
    let left_target = hit_start.saturating_sub(side_budget).max(scope_start);
    let right_target = hit_end.saturating_add(side_budget).min(scope_end);
    if !fits(&reader, hit_start, hit_end, options)? {
        return Err(QztError::ResourceLimitExceeded);
    }
    let mut scan_start = fit_left(&reader, left_target, hit_start, hit_end, options)?;
    let scan_end = fit_right(&reader, scan_start, hit_end, right_target, options)?;
    // A scan beginning immediately after LF is a complete line boundary.
    // Include the preceding byte in the same preflighted read when it fits;
    // otherwise the unknown leading edge remains a conservative budget stop.
    if scan_start > scope_start {
        let guard_start = scan_start - 1;
        if scan_end - guard_start <= options.scan && fits(&reader, guard_start, scan_end, options)?
        {
            scan_start = guard_start;
        }
    }
    let scanned = reader.read_range(scan_start, scan_end - scan_start)?;
    let scan_len = u64::try_from(scanned.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
    if scan_len != scan_end - scan_start {
        return Err(QztError::ContainerCorrupt);
    }
    let segments = segments(&scanned);
    let hit_local_start =
        usize::try_from(hit_start - scan_start).map_err(|_| QztError::ResourceLimitExceeded)?;
    let hit_local_last =
        usize::try_from(hit_end - 1 - scan_start).map_err(|_| QztError::ResourceLimitExceeded)?;
    let first = segments
        .iter()
        .position(|&(start, end)| start <= hit_local_start && hit_local_start < end)
        .ok_or(QztError::ContainerCorrupt)?;
    let last = segments
        .iter()
        .position(|&(start, end)| start <= hit_local_last && hit_local_last < end)
        .ok_or(QztError::ContainerCorrupt)?;
    let selected_first = first.saturating_sub(options.before);
    let selected_last = last.saturating_add(options.after).min(segments.len() - 1);
    let desired_start = scan_start + segments[selected_first].0 as u64;
    let desired_end = scan_start + segments[selected_last].1 as u64;
    let (excerpt_start, excerpt_end) = limit_excerpt(
        desired_start,
        desired_end,
        hit_start,
        hit_end,
        options.excerpt,
    );
    let before_returned = segments[..first]
        .iter()
        .filter(|&&(_, end)| scan_start + end as u64 > excerpt_start)
        .count();
    let after_returned = segments[last + 1..]
        .iter()
        .filter(|&&(start, _)| scan_start + (start as u64) < excerpt_end)
        .count();
    let local_start =
        usize::try_from(excerpt_start - scan_start).map_err(|_| QztError::ResourceLimitExceeded)?;
    let local_end =
        usize::try_from(excerpt_end - scan_start).map_err(|_| QztError::ResourceLimitExceeded)?;
    let leading_fragment = excerpt_start > scope_start
        && (excerpt_start == scan_start || scanned[local_start - 1] != b'\n');
    let trailing_fragment = excerpt_end < scope_end && scanned[local_end - 1] != b'\n';
    let before_stop = stop_reason(
        options.before,
        before_returned,
        excerpt_start > desired_start,
        excerpt_start == scope_start,
        leading_fragment,
    );
    let after_stop = stop_reason(
        options.after,
        after_returned,
        excerpt_end < desired_end,
        excerpt_end == scope_end,
        trailing_fragment,
    );
    let view = View {
        hit_start,
        hit_end,
        scope_start,
        scope_end,
        scope_kind,
        status,
        document,
        candidates: intersecting,
        excerpt_start,
        bytes: scanned[local_start..local_end].to_vec(),
        before_requested: options.before,
        before_returned,
        before_stop,
        after_requested: options.after,
        after_returned,
        after_stop,
        leading_fragment,
        trailing_fragment,
    };
    let output = if options.json {
        render_json(&view)
    } else {
        render_text(&view)
    };
    if u64::try_from(output.len()).map_err(|_| QztError::ResourceLimitExceeded)? > options.output {
        return Err(QztError::ResourceLimitExceeded);
    }
    Ok(output)
}

fn fits<R: ReadAt>(
    reader: &QztFileReader<R>,
    start: u64,
    end: u64,
    options: &Options,
) -> qzt::Result<bool> {
    let entries = &reader.skeleton_details().chunk_entries;
    let first = entries.partition_point(|entry| {
        entry.logical_offset.saturating_add(entry.uncompressed_size) <= start
    });
    let mut bytes = 0_u64;
    let mut count = 0_u64;
    for entry in entries[first..]
        .iter()
        .take_while(|entry| entry.logical_offset < end)
    {
        bytes = bytes
            .checked_add(entry.uncompressed_size)
            .ok_or(QztError::ResourceLimitExceeded)?;
        count += 1;
        if bytes > options.physical || count > options.chunks {
            return Ok(false);
        }
    }
    Ok(true)
}

fn fit_left(
    reader: &QztFileReader<impl ReadAt>,
    target: u64,
    hit_start: u64,
    hit_end: u64,
    options: &Options,
) -> qzt::Result<u64> {
    let (mut low, mut high) = (target, hit_start);
    while low < high {
        let mid = low + (high - low) / 2;
        if fits(reader, mid, hit_end, options)? {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    Ok(low)
}

fn fit_right(
    reader: &QztFileReader<impl ReadAt>,
    start: u64,
    hit_end: u64,
    target: u64,
    options: &Options,
) -> qzt::Result<u64> {
    let (mut low, mut high) = (hit_end, target);
    while low < high {
        let mid = low + (high - low).div_ceil(2);
        if fits(reader, start, mid, options)? {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    Ok(low)
}

fn segments(bytes: &[u8]) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut start = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            result.push((start, index + 1));
            start = index + 1;
        }
    }
    if start < bytes.len() {
        result.push((start, bytes.len()));
    }
    result
}

fn limit_excerpt(
    desired_start: u64,
    desired_end: u64,
    hit_start: u64,
    hit_end: u64,
    max: u64,
) -> (u64, u64) {
    let available = max - (hit_end - hit_start);
    let before_wanted = hit_start - desired_start;
    let after_wanted = desired_end - hit_end;
    let mut before = before_wanted.min(available / 2);
    let mut after = after_wanted.min(available - before);
    before += before_wanted
        .saturating_sub(before)
        .min(available - before - after);
    after += after_wanted
        .saturating_sub(after)
        .min(available - before - after);
    (hit_start - before, hit_end + after)
}

fn stop_reason(
    requested: usize,
    returned: usize,
    output_clipped: bool,
    reached_scope: bool,
    edge_fragment: bool,
) -> &'static str {
    if output_clipped || edge_fragment {
        "budget"
    } else if returned >= requested {
        "complete"
    } else if reached_scope {
        "scope_boundary"
    } else {
        "budget"
    }
}

fn render_json(view: &View<'_>) -> String {
    let mut out = format!(
        "{{\"hit\":{{\"logical_offset\":{},\"byte_length\":{},\"end\":{}}},\"mapping_status\":\"{}\",\"scope\":{{\"kind\":\"{}\",\"logical_offset\":{},\"end\":{}}},",
        view.hit_start,
        view.hit_end - view.hit_start,
        view.hit_end,
        view.status,
        view.scope_kind,
        view.scope_start,
        view.scope_end,
    );
    if let Some(doc) = view.document {
        let _ = write!(
            out,
            "\"document\":{{\"id\":\"{}\",\"logical_offset\":{},\"byte_length\":{},\"local_offset\":{},\"local_end\":{}}},",
            cli_json::escape(&doc.doc_id),
            doc.logical_offset,
            doc.byte_length,
            view.hit_start - doc.logical_offset,
            view.hit_end - doc.logical_offset,
        );
    } else {
        out.push_str("\"document\":null,");
    }
    out.push_str("\"candidates\":[");
    for (index, doc) in view.candidates.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"id\":\"{}\",\"logical_offset\":{},\"end\":{}}}",
            cli_json::escape(&doc.doc_id),
            doc.logical_offset,
            doc.logical_offset + doc.byte_length,
        );
    }
    let _ = writeln!(
        out,
        "],\"excerpt\":{{\"logical_offset\":{},\"byte_length\":{},\"bytes_hex\":\"{}\",\"text_escaped\":\"{}\",\"leading_fragment\":{},\"trailing_fragment\":{}}},\"before\":{{\"requested\":{},\"returned\":{},\"stop\":\"{}\"}},\"after\":{{\"requested\":{},\"returned\":{},\"stop\":\"{}\"}},\"verification\":{{\"decoded_chunks_verified\":true,\"document_index_block_verified\":{},\"document_checksum_verified\":false,\"search_query_verified\":false,\"external_provenance_verified\":false}}}}",
        view.excerpt_start,
        view.bytes.len(),
        cli_json::hex(&view.bytes),
        cli_json::escape(&escape_display(&view.bytes)),
        view.leading_fragment,
        view.trailing_fragment,
        view.before_requested,
        view.before_returned,
        view.before_stop,
        view.after_requested,
        view.after_returned,
        view.after_stop,
        view.status != "no_document_index",
    );
    out
}

fn render_text(view: &View<'_>) -> String {
    let mut out = format!(
        "hit {}:{} ({} bytes)\nmapping: {}\nscope: {} {}:{}\n",
        view.hit_start,
        view.hit_end,
        view.hit_end - view.hit_start,
        view.status,
        view.scope_kind,
        view.scope_start,
        view.scope_end,
    );
    if let Some(doc) = view.document {
        let _ = writeln!(
            out,
            "document: {} local {}:{}",
            escape_display(doc.doc_id.as_bytes()),
            view.hit_start - doc.logical_offset,
            view.hit_end - doc.logical_offset
        );
    }
    for doc in &view.candidates {
        let _ = writeln!(
            out,
            "candidate: {} {}:{}",
            escape_display(doc.doc_id.as_bytes()),
            doc.logical_offset,
            doc.logical_offset + doc.byte_length
        );
    }
    let _ = writeln!(
        out,
        "excerpt {}:{} ({} bytes; leading_fragment={}; trailing_fragment={})",
        view.excerpt_start,
        view.excerpt_start + view.bytes.len() as u64,
        view.bytes.len(),
        view.leading_fragment,
        view.trailing_fragment
    );
    let _ = writeln!(
        out,
        "before: {}/{} stop={}",
        view.before_returned, view.before_requested, view.before_stop
    );
    let _ = writeln!(
        out,
        "after: {}/{} stop={}",
        view.after_returned, view.after_requested, view.after_stop
    );
    let _ = writeln!(
        out,
        "original bytes (escaped display): {}",
        escape_display(&view.bytes)
    );
    out.push_str("verification: decoded chunks checked; document checksum, search query and external provenance not verified\n");
    out
}

fn escape_display(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut remaining = bytes;
    while !remaining.is_empty() {
        match std::str::from_utf8(remaining) {
            Ok(valid) => {
                for c in valid.chars() {
                    push_display_char(&mut out, c);
                }
                break;
            }
            Err(error) => {
                let valid = &remaining[..error.valid_up_to()];
                for c in std::str::from_utf8(valid).unwrap_or("").chars() {
                    push_display_char(&mut out, c);
                }
                remaining = &remaining[valid.len()..];
                let _ = write!(out, "\\x{:02x}", remaining[0]);
                remaining = &remaining[1..];
            }
        }
    }
    out
}

fn push_display_char(out: &mut String, c: char) {
    match c {
        '\\' => out.push_str("\\\\"),
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        c if c.is_control() || is_invisible_format(c) => {
            if (c as u32) <= 0xff {
                let _ = write!(out, "\\x{:02x}", c as u32);
            } else {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
        }
        c => out.push(c),
    }
}

fn is_invisible_format(c: char) -> bool {
    // These characters can reorder or hide adjacent log text and document IDs.
    // Escaping them leaves the separate bytes_hex recovery channel untouched.
    matches!(
        c as u32,
        0x00ad
            | 0x034f
            | 0x061c
            | 0x180e
            | 0x200b..=0x200f
            | 0x2028..=0x202e
            | 0x2060..=0x206f
            | 0xfeff
            | 0xfff9..=0xfffb
            | 0x1d173..=0x1d17a
            | 0xe0001
            | 0xe0020..=0xe007f
    )
}
