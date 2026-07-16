mod cli_json;

use std::fmt;
use std::io::{Read, Write};
use std::process::ExitCode;

use qzt::{
    Checksum, NgramIndexBuildOptions, QziFileSidecar, QztError, QztFileReader, QztFileWriter,
    RawNgramIndex, RawTokenIndex, SearchIndexSource, SearchOptions, SearchReport, SidecarIndexKind,
    TokenIndexBuildOptions, VerifyLevel, VerifyReport, WriterOptions,
    build_search_sidecar_from_file, pack_bytes_with_profile,
};

type CliResult<T> = std::result::Result<T, CliError>;

#[derive(Debug)]
enum CliError {
    Io(std::io::Error),
    Qzt(QztError),
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Qzt(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<QztError> for CliError {
    fn from(error: QztError) -> Self {
        Self::Qzt(error)
    }
}

fn main() -> ExitCode {
    // nosemgrep: rust.lang.security.args.args -- CLI dispatch is not a security boundary.
    let mut args = std::env::args();
    let _program = args.next();

    match args.next().as_deref() {
        Some("help" | "--help" | "-h") | None => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("--version" | "-V") => {
            println!("{}", qzt::version());
            ExitCode::SUCCESS
        }
        Some("pack") => run_pack(args),
        Some("info") => run_info(args),
        Some("export") => run_export(args),
        Some("range") => run_range(args),
        Some("line") => run_line(args),
        Some("docs") => run_docs(args),
        Some("doc") => run_doc(args),
        Some("search") => run_search(args),
        Some("sidecar-rebuild") => run_sidecar_rebuild(args),
        Some("verify") => run_verify(args),
        Some(command) => {
            eprintln!("qzt: unknown command '{command}'");
            eprintln!("try 'qzt --help'");
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    println!("qzt {}", qzt::version());
    println!();
    println!("Usage: qzt <COMMAND>");
    println!();
    println!("Commands:");
    println!("  help       Show this help");
    println!("  pack       Pack a UTF-8 text file into QZT");
    println!("             Use '-' as the input path to read from stdin:");
    println!("             journalctl --since today | qzt pack - -o today.qzt");
    println!("             (stdin requires --profile core without --dense-line-index;");
    println!("              stdout output is not supported; -o <path> is always required)");
    println!("  info       Print container summary (--format json for machine-readable output)");
    println!("  export     Restore original bytes (streams to -o file or stdout)");
    println!("  range      Print original bytes (--bytes A:B half-open) or lines");
    println!("             (--lines A:B 1-based inclusive)");
    println!("  line       Print one original line (1-based; --zero-based to switch)");
    println!(
        "  docs       List documents in a Document Index (--format json for machine-readable)"
    );
    println!("  doc        Extract one document (verified by default; --no-verify to skip)");
    println!(
        "  search     Search raw UTF-8 tokens with verified original-byte hits (--format json)"
    );
    println!("  sidecar-rebuild  Rebuild a QZI search sidecar (requires -o output.qzi)");
    println!("  verify     Verify container integrity (--format json for machine-readable output)");
    println!();
    println!("Options:");
    println!("  -h, --help     Show this help");
    println!("  -V, --version  Show version");
    println!();
    println!("Exit codes:");
    println!("  0  success (verify: container is valid)");
    println!("  1  command failed (verify: container is corrupt or unreadable)");
    println!("  2  usage error (unknown option / missing argument)");
}

/// Exact profile list line; kept in sync with `tests/cli_help.rs` (issue #71).
const PACK_PROFILES_LINE: &str = "Profiles: minimal, core, log, archive, memory";

fn print_pack_help() {
    println!("qzt {}", qzt::version());
    println!();
    println!(
        "Pack a UTF-8 text file into a QZT container (v0.1 technical preview; not production-ready)."
    );
    println!();
    println!("Usage: qzt pack [OPTIONS] <INPUT>");
    println!();
    println!("{PACK_PROFILES_LINE}");
    println!();
    println!("Options:");
    println!("  -o, --output <PATH>          Output .qzt path (required)");
    println!("  --profile <PROFILE>          Pack profile (default: core)");
    println!("  --chunk-size <BYTES>         Target chunk size");
    println!("  --max-chunk-size <BYTES>     Maximum chunk size");
    println!("  --zstd-level <LEVEL>         Zstd compression level");
    println!("  --checksum blake3            Checksum algorithm (only blake3 is supported)");
    println!("  --dict none                  Dictionary mode (CLI writing not implemented)");
    println!("  --dense-line-index on|off    Dense line index (default: on for memory profile)");
    println!("  -h, --help                   Show this help");
    println!();
    println!("stdin:");
    println!("  Use '-' as INPUT to read from stdin:");
    println!("  journalctl --since today | qzt pack - -o today.qzt");
    println!("  (stdin requires --profile core without --dense-line-index;");
    println!("   stdout output is not supported; -o <path> is always required)");
}

fn run_pack(args: impl Iterator<Item = String>) -> ExitCode {
    let args: Vec<String> = args.collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_pack_help();
        return ExitCode::SUCCESS;
    }

    let mut args = args.into_iter();
    let Some(input_path) = args.next() else {
        eprintln!("qzt pack: missing input file");
        return ExitCode::from(2);
    };

    let mut output_path = None;
    let mut options = WriterOptions::default();
    let mut profile = String::from("core");
    let mut dense_line_index = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" | "--output" => {
                let Some(path) = args.next() else {
                    eprintln!("qzt pack: missing output path");
                    return ExitCode::from(2);
                };
                output_path = Some(path);
            }
            "--chunk-size" => {
                let Some(size) = args.next().and_then(|value| value.parse::<usize>().ok()) else {
                    eprintln!("qzt pack: invalid --chunk-size");
                    return ExitCode::from(2);
                };
                options.chunker.target_chunk_size = size;
            }
            "--max-chunk-size" => {
                let Some(size) = args.next().and_then(|value| value.parse::<usize>().ok()) else {
                    eprintln!("qzt pack: invalid --max-chunk-size");
                    return ExitCode::from(2);
                };
                options.chunker.max_chunk_size = size;
            }
            "--zstd-level" => {
                let Some(level) = args.next().and_then(|value| value.parse::<i32>().ok()) else {
                    eprintln!("qzt pack: invalid --zstd-level");
                    return ExitCode::from(2);
                };
                options.zstd_level = level;
            }
            "--profile" => {
                let Some(value) = args.next() else {
                    eprintln!("qzt pack: missing --profile value");
                    return ExitCode::from(2);
                };
                if !matches!(
                    value.as_str(),
                    "minimal" | "core" | "log" | "archive" | "memory"
                ) {
                    eprintln!("qzt pack: invalid --profile value");
                    return ExitCode::from(2);
                }
                profile = value;
            }
            "--checksum" => {
                if args.next().as_deref() != Some("blake3") {
                    eprintln!("qzt pack: only blake3 checksum is supported");
                    return ExitCode::from(2);
                }
            }
            "--dict" => {
                if args.next().as_deref() != Some("none") {
                    eprintln!("qzt pack: CLI dictionary writing is not implemented");
                    return ExitCode::from(2);
                }
            }
            "--dense-line-index" => {
                let Some(value) = args.next() else {
                    eprintln!("qzt pack: missing --dense-line-index value");
                    return ExitCode::from(2);
                };
                if !matches!(value.as_str(), "on" | "off") {
                    eprintln!("qzt pack: invalid --dense-line-index value");
                    return ExitCode::from(2);
                }
                dense_line_index = Some(value == "on");
            }
            _ => {
                eprintln!("qzt pack: unknown option '{arg}'");
                return ExitCode::from(2);
            }
        }
    }

    let Some(output_path) = output_path else {
        eprintln!("qzt pack: missing -o output.qzt");
        return ExitCode::from(2);
    };
    if let Err(error) = options.chunker.validate() {
        eprintln!("qzt pack: {error}");
        return ExitCode::from(2);
    }

    let stdin_input = input_path == "-";
    let dense_line_index = dense_line_index.unwrap_or(profile == "memory");

    // stdin is only supported on the streaming path (core profile, no dense line index).
    // Silently buffering all of stdin would defeat the memory-safety promise for large logs.
    // Profile is checked before Dense Line Index so memory's default DLI does not mask the cause.
    if stdin_input && profile != "core" {
        eprintln!(
            "qzt pack: stdin does not support --profile {profile}; use --profile core on the streaming pack path"
        );
        if profile == "memory" {
            eprintln!(
                "(for memory profile, use the writer API pack_bytes_with_memory_profile with file-backed input)"
            );
        } else {
            eprintln!("(other profiles need the whole input in memory; write to a file first)");
        }
        return ExitCode::from(2);
    }
    if stdin_input && dense_line_index {
        eprintln!(
            "qzt pack: stdin does not support --dense-line-index on; Dense Line Index requires the in-memory pack path"
        );
        eprintln!("use --profile core without --dense-line-index on the streaming pack path");
        return ExitCode::from(2);
    }

    let result: CliResult<()> = (|| {
        if profile == "core" && !dense_line_index {
            let mut input: Box<dyn Read> = if stdin_input {
                Box::new(std::io::stdin().lock())
            } else {
                Box::new(std::fs::File::open(&input_path)?)
            };
            let temp_output_path = format!("{output_path}.tmp");
            let stream_result: CliResult<()> = (|| {
                let output = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&temp_output_path)?;
                let mut writer = QztFileWriter::new(output, options)?;
                let mut buffer = vec![0_u8; 64 * 1024];
                loop {
                    let read = input.read(&mut buffer)?;
                    if read == 0 {
                        break;
                    }
                    writer.push(&buffer[..read])?;
                }
                writer.finish()?;
                Ok(())
            })();
            if stream_result.is_err() {
                let _ = std::fs::remove_file(&temp_output_path);
            }
            stream_result?;
            std::fs::rename(temp_output_path, output_path)?;
        } else {
            let input = std::fs::read(input_path)?;
            let container = pack_bytes_with_profile(&input, options, &profile, dense_line_index)?;
            std::fs::write(output_path, container)?;
        }
        Ok(())
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => command_failed("pack", &error),
    }
}

/// Output format requested by the caller of `qzt info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InfoFormat {
    Text,
    Json,
}

fn run_info(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt info: missing file");
        return ExitCode::from(2);
    };

    let mut format = InfoFormat::Text;
    while let Some(arg) = args.next() {
        if arg.as_str() == "--format" {
            let Some(value) = args.next() else {
                eprintln!("qzt info: missing --format value");
                return ExitCode::from(2);
            };
            match value.as_str() {
                "text" => format = InfoFormat::Text,
                "json" => format = InfoFormat::Json,
                _ => {
                    eprintln!("qzt info: unknown --format value '{value}' (expected text or json)");
                    return ExitCode::from(2);
                }
            }
        } else {
            eprintln!("qzt info: unknown option '{arg}'");
            return ExitCode::from(2);
        }
    }

    let result: CliResult<_> = (|| {
        let compressed_size = std::fs::metadata(&path)?.len();
        let reader = QztFileReader::open_path(&path)?;
        let details = reader.skeleton_details();
        let metadata = details.metadata.clone();
        let document_count = details
            .document_index
            .as_ref()
            .map_or(0, |index| index.documents.len());
        Ok((reader.info(), metadata, compressed_size, document_count))
    })();

    match result {
        Ok((info, metadata, compressed_size, document_count)) => {
            let line_index = if metadata.dense_line_index {
                "sparse+dense"
            } else {
                "sparse"
            };
            // Text output: existing lines unchanged, then three new lines appended.
            if format == InfoFormat::Text {
                println!("Format: QZT 0.1");
                println!("Profile: {}", metadata.profile);
                println!("Original size: {}", info.original_size);
                println!("Compressed size: {compressed_size}");
                println!("Chunks: {}", info.chunk_count);
                println!("Lines: {}", info.line_count);
                println!("Compression: zstd");
                println!("Zstd level: {}", metadata.zstd_level);
                println!("Target chunk size: {}", metadata.target_chunk_size);
                println!("Max chunk size: {}", metadata.max_chunk_size);
                println!("Line index: {line_index}");
                println!(
                    "Document index: {}",
                    if metadata.document_index { "yes" } else { "no" }
                );
                println!("Checksum: blake3");
                println!("Zstd stream compatible: no");
                // New lines for container identity and original checksum.
                println!("Container ID: {}", cli_json::hex(&info.container_id));
                println!(
                    "Original checksum: {}:{}",
                    metadata.original_checksum.algorithm,
                    cli_json::hex(&metadata.original_checksum.value),
                );
                println!("Newline mode: {}", metadata.newline_mode);
            } else {
                // JSON output: single object on stdout.
                let container_id_hex = cli_json::hex(&info.container_id);
                let checksum_alg = cli_json::escape(&metadata.original_checksum.algorithm);
                let checksum_value = cli_json::hex(&metadata.original_checksum.value);
                let profile = cli_json::escape(&metadata.profile);
                let newline_mode = cli_json::escape(&metadata.newline_mode);
                println!(
                    concat!(
                        "{{\n",
                        "  \"format\": \"qzt-0.1\",\n",
                        "  \"container_id\": \"{container_id}\",\n",
                        "  \"profile\": \"{profile}\",\n",
                        "  \"original_size\": {original_size},\n",
                        "  \"compressed_size\": {compressed_size},\n",
                        "  \"original_checksum\": {{\"algorithm\": \"{alg}\", \"value\": \"{chk}\"}},\n",
                        "  \"newline_mode\": \"{newline_mode}\",\n",
                        "  \"chunk_count\": {chunk_count},\n",
                        "  \"line_count\": {line_count},\n",
                        "  \"zstd_level\": {zstd_level},\n",
                        "  \"target_chunk_size\": {target_chunk_size},\n",
                        "  \"max_chunk_size\": {max_chunk_size},\n",
                        "  \"dense_line_index\": {dense_line_index},\n",
                        "  \"document_index\": {document_index},\n",
                        "  \"document_count\": {document_count}\n",
                        "}}"
                    ),
                    container_id = container_id_hex,
                    profile = profile,
                    original_size = info.original_size,
                    compressed_size = compressed_size,
                    alg = checksum_alg,
                    chk = checksum_value,
                    newline_mode = newline_mode,
                    chunk_count = info.chunk_count,
                    line_count = info.line_count,
                    zstd_level = metadata.zstd_level,
                    target_chunk_size = metadata.target_chunk_size,
                    max_chunk_size = metadata.max_chunk_size,
                    dense_line_index = metadata.dense_line_index,
                    document_index = metadata.document_index,
                    document_count = document_count,
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => command_failed("info", &error),
    }
}

fn run_export(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt export: missing file");
        return ExitCode::from(2);
    };

    let mut output_path = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" | "--output" => {
                let Some(path) = args.next() else {
                    eprintln!("qzt export: missing output path");
                    return ExitCode::from(2);
                };
                output_path = Some(path);
            }
            _ => {
                eprintln!("qzt export: unknown option '{arg}'");
                return ExitCode::from(2);
            }
        }
    }

    let result: CliResult<()> = (|| {
        let reader = QztFileReader::open_path(&path)?;
        if let Some(output_path) = &output_path {
            let file = std::fs::File::create(output_path)?;
            let mut writer = std::io::BufWriter::new(file);
            reader.export_to(&mut writer)?;
            writer.flush()?;
        } else {
            let stdout = std::io::stdout();
            let mut writer = std::io::BufWriter::new(stdout.lock());
            reader.export_to(&mut writer)?;
            writer.flush()?;
        }
        Ok(())
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => command_failed("export", &error),
    }
}

fn run_range(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt range: missing file");
        return ExitCode::from(2);
    };
    let Some(flag) = args.next() else {
        eprintln!("qzt range: missing --bytes A:B or --lines A:B");
        return ExitCode::from(2);
    };
    if flag != "--bytes" && flag != "--lines" {
        eprintln!("qzt range: expected --bytes A:B or --lines A:B");
        return ExitCode::from(2);
    }
    let Some(range) = args.next() else {
        eprintln!("qzt range: missing range");
        return ExitCode::from(2);
    };

    let result: CliResult<Vec<u8>> = if flag == "--bytes" {
        let Some((start, end)) = parse_range(&range) else {
            eprintln!("qzt range: invalid byte range");
            return ExitCode::from(2);
        };
        (|| {
            let reader = QztFileReader::open_path(&path)?;
            Ok(reader.read_range(start, end.saturating_sub(start))?)
        })()
    } else {
        let Some((start, end)) = parse_line_range(&range) else {
            eprintln!("qzt range: invalid line range");
            return ExitCode::from(2);
        };
        (|| {
            let reader = QztFileReader::open_path(&path)?;
            Ok(read_line_range_file(&reader, start, end)?)
        })()
    };

    match result {
        Ok(bytes) => write_stdout(&bytes),
        Err(error) => command_failed("range", &error),
    }
}

/// Output format requested by the caller of `qzt search`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchFormat {
    Text,
    Json,
}

/// Prints the text-mode search report to stdout.
///
/// The output is byte-identical to the pre-existing format. Each hit is on its
/// own line followed by a single `metrics` line and an optional stderr warning
/// when `incomplete_reason` is set.
fn print_search_report_text(report: &SearchReport) {
    for hit in &report.hits {
        println!(
            "hit logical_offset={} byte_length={} chunk_start={} chunk_end={} source={}",
            hit.logical_offset, hit.byte_length, hit.chunk_start, hit.chunk_end, hit.source
        );
    }
    println!(
        "metrics query={} index_kind={} posting_granularity={} index_size_bytes={} source_size_bytes={} index_size_ratio={:.6} term_lookups={} posting_bytes_read={} candidate_granules={} candidate_chunks={} decoded_bytes={} physical_decoded_bytes={} verified_matches={} query_time_ms={:.3} capped={} incomplete_reason={}",
        report.metrics.query,
        report.metrics.index_kind,
        report.metrics.posting_granularity,
        report.metrics.index_size_bytes,
        report.metrics.source_size_bytes,
        report.metrics.index_size_ratio,
        report.metrics.term_lookups,
        report.metrics.posting_bytes_read,
        report.metrics.candidate_granules,
        report.metrics.candidate_chunks,
        report.metrics.decoded_bytes,
        report.metrics.physical_decoded_bytes,
        report.metrics.verified_matches,
        report.metrics.query_time_ms,
        report.capped,
        report.incomplete_reason.unwrap_or("none")
    );
    if let Some(reason) = report.incomplete_reason {
        eprintln!("qzt search: warning: result may be incomplete ({reason})");
    }
}

/// Prints the JSON-mode search report to stdout.
///
/// Hits are written one element at a time so that large result sets do not
/// require building a single giant `String`. The `source` field is passed
/// through `cli_json::escape` because it is a `&'static str` from library code
/// and is safe in practice, but the issue requires escaping it for correctness.
/// `incomplete_reason` is JSON `null` when absent and a quoted string when set.
/// The stderr warning for incomplete results is still emitted in JSON mode so
/// that the stdout JSON stream remains clean.
///
/// Note: the `score` field from [`SearchHit`] is intentionally omitted from
/// JSON output — it is always `None` in the current implementation and is also
/// absent from text output.
fn print_search_report_json(report: &SearchReport) {
    let query_escaped = cli_json::escape(&report.metrics.query);
    let index_kind_escaped = cli_json::escape(report.metrics.index_kind);
    let granularity_escaped = cli_json::escape(report.metrics.posting_granularity);

    print!("{{\"hits\":[");
    for (i, hit) in report.hits.iter().enumerate() {
        if i > 0 {
            print!(",");
        }
        let source_escaped = cli_json::escape(hit.source);
        print!(
            concat!(
                "{{",
                "\"logical_offset\":{logical_offset},",
                "\"byte_length\":{byte_length},",
                "\"chunk_start\":{chunk_start},",
                "\"chunk_end\":{chunk_end},",
                "\"source\":\"{source}\"",
                "}}"
            ),
            logical_offset = hit.logical_offset,
            byte_length = hit.byte_length,
            chunk_start = hit.chunk_start,
            chunk_end = hit.chunk_end,
            source = source_escaped,
        );
    }
    let incomplete_json = match report.incomplete_reason {
        None => "null".to_owned(),
        Some(reason) => format!("\"{}\"", cli_json::escape(reason)),
    };
    // Guard against NaN/inf producing invalid JSON for the f64 metric fields.
    debug_assert!(report.metrics.index_size_ratio.is_finite());
    debug_assert!(report.metrics.query_time_ms.is_finite());
    println!(
        concat!(
            "],",
            "\"metrics\":{{",
            "\"query\":\"{query}\",",
            "\"index_kind\":\"{index_kind}\",",
            "\"posting_granularity\":\"{posting_granularity}\",",
            "\"index_size_bytes\":{index_size_bytes},",
            "\"source_size_bytes\":{source_size_bytes},",
            "\"index_size_ratio\":{index_size_ratio},",
            "\"term_lookups\":{term_lookups},",
            "\"posting_bytes_read\":{posting_bytes_read},",
            "\"candidate_granules\":{candidate_granules},",
            "\"candidate_chunks\":{candidate_chunks},",
            "\"decoded_bytes\":{decoded_bytes},",
            "\"physical_decoded_bytes\":{physical_decoded_bytes},",
            "\"verified_matches\":{verified_matches},",
            "\"query_time_ms\":{query_time_ms}",
            "}},",
            "\"capped\":{capped},",
            "\"incomplete_reason\":{incomplete_reason}",
            "}}"
        ),
        query = query_escaped,
        index_kind = index_kind_escaped,
        posting_granularity = granularity_escaped,
        index_size_bytes = report.metrics.index_size_bytes,
        source_size_bytes = report.metrics.source_size_bytes,
        index_size_ratio = report.metrics.index_size_ratio,
        term_lookups = report.metrics.term_lookups,
        posting_bytes_read = report.metrics.posting_bytes_read,
        candidate_granules = report.metrics.candidate_granules,
        candidate_chunks = report.metrics.candidate_chunks,
        decoded_bytes = report.metrics.decoded_bytes,
        physical_decoded_bytes = report.metrics.physical_decoded_bytes,
        verified_matches = report.metrics.verified_matches,
        query_time_ms = report.metrics.query_time_ms,
        capped = report.capped,
        incomplete_reason = incomplete_json,
    );
    if let Some(reason) = report.incomplete_reason {
        eprintln!("qzt search: warning: result may be incomplete ({reason})");
    }
}

/// Output format requested by the caller of `qzt verify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifyFormat {
    Text,
    Json,
}

/// Converts a [`VerifyLevel`] to the lowercase CLI flag string used in JSON output.
///
/// Uses a match instead of `Display` on the library type to keep the conversion
/// CLI-local; `VerifyLevel` intentionally does not implement `Display`.
/// The `_` arm is required because `VerifyLevel` is `#[non_exhaustive]`.
fn verify_level_as_str(level: VerifyLevel) -> &'static str {
    match level {
        VerifyLevel::Quick => "quick",
        VerifyLevel::Normal => "normal",
        VerifyLevel::Deep => "deep",
        _ => "unknown",
    }
}

fn run_verify(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt verify: missing file");
        return ExitCode::from(2);
    };

    let mut level = VerifyLevel::Normal;
    let mut format = VerifyFormat::Text;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--quick" => level = VerifyLevel::Quick,
            "--normal" => level = VerifyLevel::Normal,
            "--deep" => level = VerifyLevel::Deep,
            "--format" => {
                let Some(value) = args.next() else {
                    eprintln!("qzt verify: missing --format value");
                    return ExitCode::from(2);
                };
                match value.as_str() {
                    "text" => format = VerifyFormat::Text,
                    "json" => format = VerifyFormat::Json,
                    _ => {
                        eprintln!(
                            "qzt verify: unknown --format value '{value}' (expected text or json)"
                        );
                        return ExitCode::from(2);
                    }
                }
            }
            _ => {
                eprintln!("qzt verify: unknown option '{arg}'");
                return ExitCode::from(2);
            }
        }
    }

    let result: CliResult<VerifyReport> = (|| {
        let reader = QztFileReader::open_path(path)?;
        Ok(reader.verify(level)?)
    })();

    match result {
        Ok(report) => {
            let level_str = verify_level_as_str(level);
            if format == VerifyFormat::Text {
                // First line is byte-identical to the pre-existing output for script
                // compatibility; report lines are appended below it.
                println!("Verify: {:?} ok", report.level);
                println!("Checked chunks: {}", report.checked_chunks);
                println!("Decoded bytes: {}", report.decoded_bytes);
            } else {
                let chunks = report.checked_chunks;
                let bytes = report.decoded_bytes;
                println!(
                    "{{\"ok\":true,\"level\":\"{level_str}\",\"checked_chunks\":{chunks},\"decoded_bytes\":{bytes}}}"
                );
            }
            ExitCode::SUCCESS
        }
        Err(ref error) => {
            if format == VerifyFormat::Json {
                // JSON consumers read stdout only; no stderr output in JSON mode.
                let error_msg = cli_json::escape(&error.to_string());
                let level_str = verify_level_as_str(level);
                println!("{{\"ok\":false,\"level\":\"{level_str}\",\"error\":\"{error_msg}\"}}");
                ExitCode::from(1)
            } else {
                command_failed("verify", error)
            }
        }
    }
}

fn run_line(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt line: missing file");
        return ExitCode::from(2);
    };
    let Some(line) = args.next() else {
        eprintln!("qzt line: missing line number");
        return ExitCode::from(2);
    };
    let zero_based = args.any(|arg| arg == "--zero-based");
    let Ok(mut line_number) = line.parse::<u64>() else {
        eprintln!("qzt line: invalid line number");
        return ExitCode::from(2);
    };
    if !zero_based {
        if line_number == 0 {
            eprintln!("qzt line: line numbers are 1-based by default");
            return ExitCode::from(2);
        }
        line_number -= 1;
    }

    let result: CliResult<Vec<u8>> = (|| {
        let reader = QztFileReader::open_path(path)?;
        Ok(reader.read_line_raw(line_number)?)
    })();

    match result {
        Ok(bytes) => write_stdout(&bytes),
        Err(error) => command_failed("line", &error),
    }
}

/// Output format for `qzt docs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocsFormat {
    Text,
    Json,
}

/// Escapes a `doc_id` for tab-separated text output.
///
/// Replaces literal tab characters with `\t` and newlines with `\n` so that
/// the tab-separated columns remain unambiguous when a `doc_id` contains those
/// characters.
fn escape_doc_id_text(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for ch in id.chars() {
        match ch {
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

fn run_docs(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt docs: missing file");
        return ExitCode::from(2);
    };

    let mut format = DocsFormat::Text;
    while let Some(arg) = args.next() {
        if arg.as_str() == "--format" {
            let Some(value) = args.next() else {
                eprintln!("qzt docs: missing --format value");
                return ExitCode::from(2);
            };
            match value.as_str() {
                "text" => format = DocsFormat::Text,
                "json" => format = DocsFormat::Json,
                _ => {
                    eprintln!("qzt docs: unknown --format value '{value}' (expected text or json)");
                    return ExitCode::from(2);
                }
            }
        } else {
            eprintln!("qzt docs: unknown option '{arg}'");
            return ExitCode::from(2);
        }
    }

    let result: CliResult<_> = (|| {
        let reader = QztFileReader::open_path(&path)?;
        let details = reader.skeleton_details();
        let document_index = details
            .document_index
            .as_ref()
            .ok_or(QztError::MissingRequiredBlock)?;
        Ok(document_index.documents.clone())
    })();

    match result {
        Ok(documents) => {
            if format == DocsFormat::Text {
                println!("doc_id\toffset\tbytes\tfirst_line\tlines\tchecksum");
                for doc in &documents {
                    let doc_id_escaped = escape_doc_id_text(&doc.doc_id);
                    let checksum_hex = cli_json::hex(&doc.checksum.value);
                    let first_line_one_based = doc.first_line.saturating_add(1);
                    println!(
                        "{}\t{}\t{}\t{}\t{}\t{}:{}",
                        doc_id_escaped,
                        doc.logical_offset,
                        doc.byte_length,
                        first_line_one_based,
                        doc.line_count,
                        cli_json::escape(&doc.checksum.algorithm),
                        checksum_hex,
                    );
                }
            } else {
                // JSON output: {"documents":[...]}
                print!("{{\"documents\":[");
                for (i, doc) in documents.iter().enumerate() {
                    if i > 0 {
                        print!(",");
                    }
                    let doc_id_json = cli_json::escape(&doc.doc_id);
                    let alg_json = cli_json::escape(&doc.checksum.algorithm);
                    let checksum_hex = cli_json::hex(&doc.checksum.value);
                    let first_line_one_based = doc.first_line.saturating_add(1);
                    print!(
                        concat!(
                            "{{",
                            "\"doc_id\":\"{doc_id}\",",
                            "\"logical_offset\":{offset},",
                            "\"byte_length\":{length},",
                            "\"first_line\":{first_line},",
                            "\"line_count\":{line_count},",
                            "\"checksum\":{{\"algorithm\":\"{alg}\",\"value\":\"{chk}\"}}",
                            "}}"
                        ),
                        doc_id = doc_id_json,
                        offset = doc.logical_offset,
                        length = doc.byte_length,
                        first_line = first_line_one_based,
                        line_count = doc.line_count,
                        alg = alg_json,
                        chk = checksum_hex,
                    );
                }
                println!("]}}");
            }
            ExitCode::SUCCESS
        }
        Err(ref error) => {
            eprintln!("qzt docs: {error}");
            ExitCode::from(1)
        }
    }
}

fn run_doc(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt doc: missing file");
        return ExitCode::from(2);
    };
    let Some(doc_id) = args.next() else {
        eprintln!("qzt doc: missing doc_id");
        return ExitCode::from(2);
    };

    let mut output_path: Option<String> = None;
    let mut no_verify = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" | "--output" => {
                let Some(p) = args.next() else {
                    eprintln!("qzt doc: missing output path");
                    return ExitCode::from(2);
                };
                output_path = Some(p);
            }
            "--no-verify" => {
                no_verify = true;
            }
            _ => {
                eprintln!("qzt doc: unknown option '{arg}'");
                return ExitCode::from(2);
            }
        }
    }

    let result: CliResult<Vec<u8>> = (|| {
        let reader = QztFileReader::open_path(&path)?;
        let bytes = if no_verify {
            reader.read_document(&doc_id)?
        } else {
            // Look up the expected checksum from the Document Index entry.
            let details = reader.skeleton_details();
            let document_index = details
                .document_index
                .as_ref()
                .ok_or(QztError::MissingRequiredBlock)?;
            let pos = details
                .document_lookup
                .get(doc_id.as_str())
                .copied()
                .ok_or(QztError::DocumentNotFound)?;
            let entry = &document_index.documents[pos];
            let expected = Checksum {
                algorithm: entry.checksum.algorithm.clone(),
                value: entry.checksum.value,
            };
            reader.read_document_verified(&doc_id, &expected)?
        };
        Ok(bytes)
    })();

    match result {
        Ok(bytes) => {
            if let Some(ref out_path) = output_path {
                match std::fs::write(out_path, &bytes) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(error) => {
                        eprintln!("qzt doc: {error}");
                        ExitCode::from(1)
                    }
                }
            } else {
                write_stdout(&bytes)
            }
        }
        Err(ref error) => {
            match error {
                CliError::Qzt(QztError::MissingRequiredBlock) => {
                    eprintln!("qzt doc: no Document Index in this container");
                }
                CliError::Qzt(QztError::DocumentNotFound) => {
                    eprintln!("qzt doc: doc_id not found: {doc_id}");
                }
                _ => eprintln!("qzt doc: {error}"),
            }
            ExitCode::from(1)
        }
    }
}

fn run_search(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt search: missing file");
        return ExitCode::from(2);
    };
    let Some(query) = args.next() else {
        eprintln!("qzt search: missing query");
        return ExitCode::from(2);
    };

    let mut options = SearchOptions::default();
    let mut index_kind = "token";
    let mut ngram = 3_usize;
    let mut sidecar_path = None;
    let mut format = SearchFormat::Text;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--index" => {
                let Some(value) = args.next() else {
                    eprintln!("qzt search: missing --index value");
                    return ExitCode::from(2);
                };
                if !matches!(value.as_str(), "token" | "ngram") {
                    eprintln!("qzt search: invalid --index value");
                    return ExitCode::from(2);
                }
                index_kind = if value == "ngram" { "ngram" } else { "token" };
            }
            "--ngram" => {
                let Some(value) = args.next().and_then(|value| value.parse::<usize>().ok()) else {
                    eprintln!("qzt search: invalid --ngram");
                    return ExitCode::from(2);
                };
                if value == 0 {
                    eprintln!("qzt search: invalid --ngram");
                    return ExitCode::from(2);
                }
                ngram = value;
            }
            "--sidecar" => {
                let Some(path) = args.next() else {
                    eprintln!("qzt search: missing --sidecar path");
                    return ExitCode::from(2);
                };
                sidecar_path = Some(path);
            }
            "--max-candidates" => {
                let Some(value) = args.next().and_then(|value| value.parse::<u64>().ok()) else {
                    eprintln!("qzt search: invalid --max-candidates");
                    return ExitCode::from(2);
                };
                options.max_candidate_granules = value;
            }
            "--max-decoded-bytes" => {
                let Some(value) = args.next().and_then(|value| parse_byte_limit(&value)) else {
                    eprintln!("qzt search: invalid --max-decoded-bytes");
                    return ExitCode::from(2);
                };
                options.max_decoded_bytes = value;
            }
            "--max-results" => {
                let Some(value) = args.next().and_then(|value| value.parse::<u64>().ok()) else {
                    eprintln!("qzt search: invalid --max-results");
                    return ExitCode::from(2);
                };
                options.max_search_results = value;
            }
            "--format" => {
                let Some(value) = args.next() else {
                    eprintln!("qzt search: missing --format value");
                    return ExitCode::from(2);
                };
                match value.as_str() {
                    "text" => format = SearchFormat::Text,
                    "json" => format = SearchFormat::Json,
                    _ => {
                        eprintln!(
                            "qzt search: unknown --format value '{value}' (expected text or json)"
                        );
                        return ExitCode::from(2);
                    }
                }
            }
            _ => {
                eprintln!("qzt search: unknown option '{arg}'");
                return ExitCode::from(2);
            }
        }
    }

    let result: CliResult<_> = (|| {
        let reader = QztFileReader::open_path(&path)?;
        if let Some(sidecar_path) = &sidecar_path {
            let sidecar = QziFileSidecar::open_path(sidecar_path, &reader)?;
            Ok(sidecar.search(&reader, &query, options)?)
        } else if index_kind == "ngram" {
            let index = RawNgramIndex::build_from_file(
                &reader,
                NgramIndexBuildOptions {
                    source: SearchIndexSource::RawUtf8,
                    n: ngram,
                    ..NgramIndexBuildOptions::default()
                },
            )?;
            Ok(index.search_file(&reader, &query, options)?)
        } else {
            let index = RawTokenIndex::build_from_file(&reader, TokenIndexBuildOptions::default())?;
            Ok(index.search_file(&reader, &query, options)?)
        }
    })();

    match result {
        Ok(report) => {
            match format {
                SearchFormat::Text => print_search_report_text(&report),
                SearchFormat::Json => print_search_report_json(&report),
            }
            ExitCode::SUCCESS
        }
        Err(error) => command_failed("search", &error),
    }
}

fn run_sidecar_rebuild(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt sidecar-rebuild: missing file");
        return ExitCode::from(2);
    };

    let mut output_path = None;
    let mut index_kind = "token";
    let mut ngram = 3_usize;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" | "--output" => {
                let Some(path) = args.next() else {
                    eprintln!("qzt sidecar-rebuild: missing output path");
                    return ExitCode::from(2);
                };
                output_path = Some(path);
            }
            "--index" => {
                let Some(value) = args.next() else {
                    eprintln!("qzt sidecar-rebuild: missing --index value");
                    return ExitCode::from(2);
                };
                if !matches!(value.as_str(), "token" | "ngram") {
                    eprintln!("qzt sidecar-rebuild: invalid --index value");
                    return ExitCode::from(2);
                }
                index_kind = if value == "ngram" { "ngram" } else { "token" };
            }
            "--ngram" => {
                let Some(value) = args.next().and_then(|value| value.parse::<usize>().ok()) else {
                    eprintln!("qzt sidecar-rebuild: invalid --ngram");
                    return ExitCode::from(2);
                };
                if value == 0 {
                    eprintln!("qzt sidecar-rebuild: invalid --ngram");
                    return ExitCode::from(2);
                }
                ngram = value;
            }
            _ => {
                eprintln!("qzt sidecar-rebuild: unknown option '{arg}'");
                return ExitCode::from(2);
            }
        }
    }

    let Some(output_path) = output_path else {
        eprintln!("qzt sidecar-rebuild: missing -o output.qzi");
        return ExitCode::from(2);
    };

    let kind = if index_kind == "ngram" {
        SidecarIndexKind::Ngram { n: ngram }
    } else {
        SidecarIndexKind::Token
    };
    let result: CliResult<()> = (|| {
        let reader = QztFileReader::open_path(&path)?;
        let sidecar = build_search_sidecar_from_file(&reader, kind)?;
        std::fs::write(output_path, sidecar)?;
        Ok(())
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => command_failed("sidecar-rebuild", &error),
    }
}

fn parse_range(range: &str) -> Option<(u64, u64)> {
    let (start, end) = range.split_once(':')?;
    let start = start.parse().ok()?;
    let end = end.parse().ok()?;
    (start <= end).then_some((start, end))
}

fn parse_line_range(range: &str) -> Option<(u64, u64)> {
    let (start, end) = parse_range(range)?;
    (start > 0 && start <= end).then_some((start, end))
}

fn parse_byte_limit(value: &str) -> Option<u64> {
    if let Some(number) = value.strip_suffix("MiB") {
        return number.parse::<u64>().ok()?.checked_mul(1024 * 1024);
    }
    if let Some(number) = value.strip_suffix("KiB") {
        return number.parse::<u64>().ok()?.checked_mul(1024);
    }
    if let Some(number) = value.strip_suffix("GiB") {
        return number.parse::<u64>().ok()?.checked_mul(1024 * 1024 * 1024);
    }
    value.parse().ok()
}

fn read_line_range_file(
    reader: &QztFileReader<std::fs::File>,
    start_one_based: u64,
    end_one_based: u64,
) -> qzt::Result<Vec<u8>> {
    let mut output = Vec::new();
    for line in start_one_based..=end_one_based {
        output.extend_from_slice(&reader.read_line_raw(line - 1)?);
    }
    Ok(output)
}

fn write_stdout(bytes: &[u8]) -> ExitCode {
    match std::io::stdout().write_all(bytes) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(1),
    }
}

fn command_failed(command: &str, error: &CliError) -> ExitCode {
    eprintln!("qzt {command}: {error}");
    ExitCode::from(1)
}
