mod cli_json;

use std::fmt;
use std::io::{Read, Write};
use std::process::ExitCode;

use qzt::{
    build_search_sidecar_from_file, pack_bytes_with_profile, NgramIndexBuildOptions,
    QziFileSidecar, QztError, QztFileReader, QztFileWriter, RawNgramIndex, RawTokenIndex,
    SearchIndexSource, SearchOptions, SidecarIndexKind, TokenIndexBuildOptions, VerifyLevel,
    WriterOptions,
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
    println!("  info       Print container summary (--format json for machine-readable output)");
    println!("  export     Restore original bytes (streams to -o file or stdout)");
    println!("  range      Print original bytes (--bytes A:B half-open) or lines");
    println!("             (--lines A:B 1-based inclusive)");
    println!("  line       Print one original line (1-based; --zero-based to switch)");
    println!("  search     Search raw UTF-8 tokens with verified original-byte hits");
    println!("  sidecar-rebuild  Rebuild a QZI search sidecar (requires -o output.qzi)");
    println!("  verify     Verify container integrity");
    println!();
    println!("Options:");
    println!("  -h, --help     Show this help");
    println!("  -V, --version  Show version");
}

fn run_pack(mut args: impl Iterator<Item = String>) -> ExitCode {
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

    let dense_line_index = dense_line_index.unwrap_or(profile == "memory");
    let result: CliResult<()> = (|| {
        if profile == "core" && !dense_line_index {
            let mut input = std::fs::File::open(input_path)?;
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
                    cli_json::escape(&metadata.original_checksum.algorithm),
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

fn run_verify(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt verify: missing file");
        return ExitCode::from(2);
    };

    let mut level = VerifyLevel::Normal;
    for arg in args {
        match arg.as_str() {
            "--quick" => level = VerifyLevel::Quick,
            "--normal" => level = VerifyLevel::Normal,
            "--deep" => level = VerifyLevel::Deep,
            _ => {
                eprintln!("qzt verify: unknown option '{arg}'");
                return ExitCode::from(2);
            }
        }
    }

    let result: CliResult<()> = (|| {
        let reader = QztFileReader::open_path(path)?;
        reader.verify(level)?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            println!("Verify: {level:?} ok");
            ExitCode::SUCCESS
        }
        Err(error) => command_failed("verify", &error),
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
