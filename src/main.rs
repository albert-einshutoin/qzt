use std::io::Write;
use std::process::ExitCode;

use qzt::reader::{QztReader, VerifyLevel};
use qzt::search::{RawTokenIndex, SearchOptions, TokenIndexBuildOptions};
use qzt::writer::{pack_bytes, WriterOptions};

fn main() -> ExitCode {
    let mut args = std::env::args();
    let _program = args.next();

    match args.next().as_deref() {
        Some("help") | Some("--help") | Some("-h") | None => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("--version") | Some("-V") => {
            println!("{}", qzt::version());
            ExitCode::SUCCESS
        }
        Some("pack") => run_pack(args),
        Some("info") => run_info(args),
        Some("export") => run_export(args),
        Some("range") => run_range(args),
        Some("line") => run_line(args),
        Some("search") => run_search(args),
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
    println!("  info       Print container summary");
    println!("  export     Restore original bytes");
    println!("  range      Print original bytes in a half-open byte range");
    println!("  line       Print one original line");
    println!("  search     Search raw UTF-8 tokens with verified original-byte hits");
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
                let Some(profile) = args.next() else {
                    eprintln!("qzt pack: missing --profile value");
                    return ExitCode::from(2);
                };
                if !matches!(
                    profile.as_str(),
                    "minimal" | "core" | "log" | "archive" | "memory"
                ) {
                    eprintln!("qzt pack: invalid --profile value");
                    return ExitCode::from(2);
                }
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

    let result = std::fs::read(input_path)
        .map_err(|_| ())
        .and_then(|input| pack_bytes(&input, options).map_err(|_| ()))
        .and_then(|container| std::fs::write(output_path, container).map_err(|_| ()));

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => {
            eprintln!("qzt pack: failed");
            ExitCode::from(1)
        }
    }
}

fn run_info(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt info: missing file");
        return ExitCode::from(2);
    };

    let result = std::fs::read(path).map_err(|_| ()).and_then(|bytes| {
        let compressed_size = bytes.len();
        QztReader::open(bytes)
            .map(|reader| (reader.info(), compressed_size))
            .map_err(|_| ())
    });

    match result {
        Ok((info, compressed_size)) => {
            println!("Format: QZT 0.1");
            println!("Profile: core");
            println!("Original size: {}", info.original_size);
            println!("Compressed size: {compressed_size}");
            println!("Chunks: {}", info.chunk_count);
            println!("Lines: {}", info.line_count);
            println!("Compression: zstd");
            println!("Line index: sparse");
            println!("Checksum: blake3");
            println!("Zstd stream compatible: no");
            ExitCode::SUCCESS
        }
        Err(()) => {
            eprintln!("qzt info: failed");
            ExitCode::from(1)
        }
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

    let result = std::fs::read(path)
        .map_err(|_| ())
        .and_then(|bytes| QztReader::open(bytes).map_err(|_| ()))
        .and_then(|reader| reader.export_all().map_err(|_| ()));

    match (result, output_path) {
        (Ok(bytes), Some(output_path)) => match std::fs::write(output_path, bytes) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => {
                eprintln!("qzt export: failed");
                ExitCode::from(1)
            }
        },
        (Ok(bytes), None) => write_stdout(&bytes),
        (Err(()), _) => {
            eprintln!("qzt export: failed");
            ExitCode::from(1)
        }
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

    let result = if flag == "--bytes" {
        let Some((start, end)) = parse_range(&range) else {
            eprintln!("qzt range: invalid byte range");
            return ExitCode::from(2);
        };
        std::fs::read(path)
            .map_err(|_| ())
            .and_then(|bytes| QztReader::open(bytes).map_err(|_| ()))
            .and_then(|reader| {
                reader
                    .read_range(start, end.saturating_sub(start))
                    .map_err(|_| ())
            })
    } else {
        let Some((start, end)) = parse_line_range(&range) else {
            eprintln!("qzt range: invalid line range");
            return ExitCode::from(2);
        };
        std::fs::read(path)
            .map_err(|_| ())
            .and_then(|bytes| QztReader::open(bytes).map_err(|_| ()))
            .and_then(|reader| read_line_range(&reader, start, end).map_err(|_| ()))
    };

    match result {
        Ok(bytes) => write_stdout(&bytes),
        Err(()) => {
            eprintln!("qzt range: failed");
            ExitCode::from(1)
        }
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

    let result = std::fs::read(path)
        .map_err(|_| ())
        .and_then(|bytes| QztReader::open(bytes).map_err(|_| ()))
        .and_then(|reader| reader.verify(level).map(|_| ()).map_err(|_| ()));

    match result {
        Ok(()) => {
            println!("Verify: {level:?} ok");
            ExitCode::SUCCESS
        }
        Err(()) => {
            eprintln!("qzt verify: failed");
            ExitCode::from(1)
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

    let result = std::fs::read(path)
        .map_err(|_| ())
        .and_then(|bytes| QztReader::open(bytes).map_err(|_| ()))
        .and_then(|reader| reader.read_line_raw(line_number).map_err(|_| ()));

    match result {
        Ok(bytes) => write_stdout(&bytes),
        Err(()) => {
            eprintln!("qzt line: failed");
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
    while let Some(arg) = args.next() {
        match arg.as_str() {
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
            _ => {
                eprintln!("qzt search: unknown option '{arg}'");
                return ExitCode::from(2);
            }
        }
    }

    let result = std::fs::read(path).map_err(|_| ()).and_then(|bytes| {
        let index = RawTokenIndex::build_from_container(&bytes, TokenIndexBuildOptions::default())
            .map_err(|_| ())?;
        let reader = QztReader::open(&bytes).map_err(|_| ())?;
        index.search(&reader, &query, options).map_err(|_| ())
    });

    match result {
        Ok(report) => {
            for hit in &report.hits {
                println!(
                    "hit logical_offset={} byte_length={} chunk_start={} chunk_end={} source={}",
                    hit.logical_offset, hit.byte_length, hit.chunk_start, hit.chunk_end, hit.source
                );
            }
            println!(
                "metrics query={} index_kind={} posting_granularity={} index_size_bytes={} source_size_bytes={} index_size_ratio={:.6} term_lookups={} posting_bytes_read={} candidate_granules={} candidate_chunks={} decoded_bytes={} verified_matches={} query_time_ms={:.3} capped={}",
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
                report.metrics.verified_matches,
                report.metrics.query_time_ms,
                report.capped
            );
            ExitCode::SUCCESS
        }
        Err(()) => {
            eprintln!("qzt search: failed");
            ExitCode::from(1)
        }
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

fn read_line_range(
    reader: &QztReader,
    start_one_based: u64,
    end_one_based: u64,
) -> qzt::error::Result<Vec<u8>> {
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
