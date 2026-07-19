mod cli_attest;
mod cli_json;

use std::collections::HashSet;
use std::fmt;
use std::io::{Read, Write};
use std::path::Path;
use std::process::ExitCode;

use qzt::{
    Checksum, DocumentSpan, NgramIndexBuildOptions, QziFileSidecar, QztError, QztFileReader,
    QztFileWriter, RawNgramIndex, RawTokenIndex, ReadAt, SearchIndexSource, SearchOptions,
    SearchReport, SidecarIndexKind, TokenIndexBuildOptions, VerifyLevel, VerifyReport,
    WriterBuilder, WriterOptions, build_search_sidecar_from_file,
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
    let command = args.next();
    let remaining: Vec<String> = args.collect();

    if let Some(command) = command.as_deref() {
        if remaining.iter().any(|arg| arg == "--help" || arg == "-h") {
            return print_command_help(command);
        }
    }

    match command.as_deref() {
        Some("help") if remaining.len() == 1 => print_command_help(&remaining[0]),
        Some("help" | "--help" | "-h") | None => print_help(),
        Some("--version" | "-V") => write_stdout(format!("qzt {}\n", qzt::version()).as_bytes()),
        Some("pack") => run_pack(remaining.into_iter()),
        Some("pack-docs") => run_pack_docs(remaining.into_iter()),
        Some("attest") => run_attest(remaining.into_iter()),
        Some("info") => run_info(remaining.into_iter()),
        Some("export") => run_export(remaining.into_iter()),
        Some("range") => run_range(remaining.into_iter()),
        Some("line") => run_line(remaining.into_iter()),
        Some("docs") => run_docs(remaining.into_iter()),
        Some("doc") => run_doc(remaining.into_iter()),
        Some("search") => run_search(remaining.into_iter()),
        Some("inspect-sidecar") => run_inspect_sidecar(remaining.into_iter()),
        Some("sidecar-rebuild") => run_sidecar_rebuild(remaining.into_iter()),
        Some("verify") => run_verify(remaining.into_iter()),
        Some(command) => {
            eprintln!("qzt: unknown command '{command}'");
            eprintln!("try 'qzt --help'");
            ExitCode::from(2)
        }
    }
}

fn print_command_help(command: &str) -> ExitCode {
    match command {
        "pack" => print_pack_help(),
        "pack-docs" => print_pack_docs_help(),
        "attest" => print_attest_help(),
        "info" => print_simple_command_help(
            "Print structural metadata.",
            "qzt info <FILE> [--format text|json]",
            "  --format <FORMAT>  Output format: text or json (default: text)",
        ),
        "export" => print_simple_command_help(
            "Restore all verified chunk bytes.",
            "qzt export <FILE> [-o <OUTPUT>]",
            "  -o, --output <PATH>  Write to a file instead of stdout",
        ),
        "range" => print_simple_command_help(
            "Restore an original byte or line range.",
            "qzt range <FILE> --bytes A:B|--lines A:B",
            concat!(
                "  --bytes <A:B>  Zero-based half-open byte interval\n",
                "  --lines <A:B>  One-based inclusive line interval"
            ),
        ),
        "line" => print_simple_command_help(
            "Restore one original line.",
            "qzt line <FILE> <LINE> [--zero-based]",
            "  --zero-based  Interpret LINE as zero-based (default: one-based)",
        ),
        "docs" => print_simple_command_help(
            "List verified Document Index entries.",
            "qzt docs <FILE> [--format text|json]",
            "  --format <FORMAT>  Output format: text or json (default: text)",
        ),
        "doc" => print_simple_command_help(
            "Restore one indexed document.",
            "qzt doc <FILE> <DOC_ID> [-o <OUTPUT>] [--no-verify]",
            concat!(
                "  -o, --output <PATH>  Write to a file instead of stdout\n",
                "  --no-verify          Skip the document checksum (diagnostic use only)"
            ),
        ),
        "search" => print_simple_command_help(
            "Search original UTF-8 bytes and verify every reported hit.",
            "qzt search <FILE> <QUERY> [OPTIONS]",
            concat!(
                "  --index token|ngram       In-memory index kind (default: token)\n",
                "  --ngram <N>               N-gram width (default: 3)\n",
                "  --sidecar <PATH>          Use an existing QZI sidecar\n",
                "  --max-candidates <N>      Candidate-granule budget\n",
                "  --max-decoded-bytes <N>   Decode budget; KiB/MiB/GiB suffixes accepted\n",
                "  --max-results <N>         Result cap\n",
                "  --format text|json        Output format (default: text)"
            ),
        ),
        "inspect-sidecar" => print_simple_command_help(
            "Inspect metadata from a validated, source-bound QZI sidecar.",
            "qzt inspect-sidecar <FILE.qzt> --sidecar <FILE.qzi> [--format text|json]",
            concat!(
                "  --sidecar <PATH>     QZI sidecar path (required)\n",
                "  --format text|json  Output format (default: text)"
            ),
        ),
        "sidecar-rebuild" => print_simple_command_help(
            "Build a rebuildable QZI search sidecar.",
            "qzt sidecar-rebuild <FILE> -o <OUTPUT.qzi> [OPTIONS]",
            concat!(
                "  -o, --output <PATH>  Output .qzi path (required)\n",
                "  --index token|ngram  Index kind (default: token)\n",
                "  --ngram <N>          N-gram width (default: 3)"
            ),
        ),
        "verify" => print_simple_command_help(
            "Verify container integrity.",
            "qzt verify <FILE> [--quick|--normal|--deep] [--format text|json]",
            concat!(
                "  --quick|--normal|--deep  Verification level (default: normal)\n",
                "  --format text|json       Output format (default: text)"
            ),
        ),
        _ => {
            eprintln!("qzt: unknown command '{command}'");
            eprintln!("try 'qzt --help'");
            ExitCode::from(2)
        }
    }
}

fn print_simple_command_help(description: &str, usage: &str, options: &str) -> ExitCode {
    let output = format!(
        "qzt {}\n\n{description}\n\nUsage: {usage}\n\nOptions:\n{options}\n  -h, --help  Show this help\n",
        qzt::version()
    );
    write_stdout(output.as_bytes())
}

fn print_help() -> ExitCode {
    let output = format!(
        concat!(
            "qzt {}\n\n",
            "Usage: qzt <COMMAND>\n\n",
            "Commands:\n",
            "  help       Show this help\n",
            "  pack       Pack a UTF-8 text file into QZT\n",
            "             Use '-' as the input path to read from stdin:\n",
            "             journalctl --since today | qzt pack - -o today.qzt\n",
            "             (stdin requires --profile core without --dense-line-index;\n",
            "              stdout output is not supported; -o <path> is always required)\n",
            "  pack-docs  Pack multiple files as verified documents in one QZT container\n",
            "  attest     Emit a verified canonical JSON attestation for signing\n",
            "  info       Print container summary (--format json for machine-readable output)\n",
            "  export     Restore original bytes (streams to -o file or stdout)\n",
            "  range      Print original bytes (--bytes A:B half-open) or lines\n",
            "             (--lines A:B 1-based inclusive)\n",
            "  line       Print one original line (1-based; --zero-based to switch)\n",
            "  docs       List documents in a Document Index (--format json for machine-readable)\n",
            "  doc        Extract one document (verified by default; --no-verify to skip)\n",
            "  search     Search raw UTF-8 tokens with verified original-byte hits (--format json)\n",
            "  inspect-sidecar  Inspect a validated QZI sidecar (--format json available)\n",
            "  sidecar-rebuild  Rebuild a QZI search sidecar (requires -o output.qzi)\n",
            "  verify     Verify container integrity (--format json for machine-readable output)\n\n",
            "Options:\n",
            "  -h, --help     Show this help\n",
            "  -V, --version  Show version\n\n",
            "Exit codes:\n",
            "  0  success (verify: container is valid)\n",
            "  1  command failed (verify: container is corrupt or unreadable)\n",
            "  2  usage error (unknown option / missing argument)\n\n",
            "Full reference and stability contract:\n",
            "https://github.com/albert-einshutoin/qzt/blob/main/docs/CLI.md\n"
        ),
        qzt::version()
    );
    write_stdout(output.as_bytes())
}

fn print_attest_help() -> ExitCode {
    let output = format!(
        concat!(
            "qzt {}\n\n",
            "Verify a QZT container and emit canonical JSON for external signing.\n\n",
            "Usage: qzt attest [OPTIONS] <FILE>\n\n",
            "Options:\n",
            "  --level <LEVEL>  Verification level: quick, normal, or deep (default: deep)\n",
            "  -h, --help       Show this help\n\n",
            "Output is one deterministic canonical JSON line followed by one newline.\n"
        ),
        qzt::version()
    );
    write_stdout(output.as_bytes())
}

fn print_pack_docs_help() -> ExitCode {
    let output = format!(
        concat!(
            "qzt {}\n\n",
            "Pack multiple UTF-8 files into one Document Index container.\n\n",
            "Usage: qzt pack-docs [OPTIONS] <INPUT>...\n\n",
            "Options:\n",
            "  -o, --output <PATH>          Output .qzt path (required)\n",
            "  --doc-id-prefix <PREFIX>     Prefix each basename document id\n",
            "  --profile <PROFILE>           Pack profile (default: core)\n",
            "  --chunk-size <BYTES>          Target chunk size\n",
            "  --max-chunk-size <BYTES>      Maximum chunk size\n",
            "  --zstd-level <LEVEL>          Zstd compression level\n",
            "  --checksum blake3             Checksum algorithm (only blake3 is supported)\n",
            "  --dict none                   Dictionary mode (CLI writing not implemented)\n",
            "  --dense-line-index on|off     Dense line index (default: on for memory profile)\n",
            "  -h, --help                    Show this help\n\n",
            "Memory: this command reads all inputs before packing and uses memory proportional to their total input size.\n",
            "        With --profile memory, the default 256 KiB target / 2 MiB maximum chunks bound small reads but may reduce compression ratio;\n",
            "        --chunk-size and --max-chunk-size override that trade-off.\n"
        ),
        qzt::version()
    );
    write_stdout(output.as_bytes())
}

/// Exact profile list line; kept in sync with `tests/cli_help.rs` (issue #71).
const PACK_PROFILES_LINE: &str = "Profiles: minimal, core, log, archive, memory";
const PACK_DOCS_MEMORY_DEFAULT_TARGET_CHUNK_SIZE: usize = 256 * 1024;
const PACK_DOCS_MEMORY_DEFAULT_MAX_CHUNK_SIZE: usize = 2 * 1024 * 1024;

fn print_pack_help() -> ExitCode {
    let output = format!(
        concat!(
            "qzt {}\n\n",
            "Pack a UTF-8 text file into a QZT container (v0.1 technical preview; not production-ready).\n\n",
            "Usage: qzt pack [OPTIONS] <INPUT>\n\n",
            "{}\n\n",
            "Options:\n",
            "  -o, --output <PATH>          Output .qzt path (required)\n",
            "  --profile <PROFILE>          Pack profile (default: core)\n",
            "  --chunk-size <BYTES>         Target chunk size\n",
            "  --max-chunk-size <BYTES>     Maximum chunk size\n",
            "  --zstd-level <LEVEL>         Zstd compression level\n",
            "  --checksum blake3            Checksum algorithm (only blake3 is supported)\n",
            "  --dict none                  Dictionary mode (CLI writing not implemented)\n",
            "  --dense-line-index on|off    Dense line index (default: on for memory profile)\n",
            "  -h, --help                   Show this help\n\n",
            "stdin:\n",
            "  Use '-' as INPUT to read from stdin:\n",
            "  journalctl --since today | qzt pack - -o today.qzt\n",
            "  (stdin requires --profile core without --dense-line-index;\n",
            "   stdout output is not supported; -o <path> is always required)\n"
        ),
        qzt::version(),
        PACK_PROFILES_LINE
    );
    write_stdout(output.as_bytes())
}

fn run_pack(args: impl Iterator<Item = String>) -> ExitCode {
    let args: Vec<String> = args.collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return print_pack_help();
    }

    let mut args = args.into_iter();
    let mut input_path = None;
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
            _ if arg.starts_with('-') && arg != "-" => {
                eprintln!("qzt pack: unknown option '{arg}'");
                return ExitCode::from(2);
            }
            _ if input_path.is_some() => {
                eprintln!("qzt pack: unexpected additional input '{arg}'");
                return ExitCode::from(2);
            }
            _ => input_path = Some(arg),
        }
    }

    let Some(input_path) = input_path else {
        eprintln!("qzt pack: missing input file");
        return ExitCode::from(2);
    };

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
                "(for memory profile, use WriterBuilder::profile(\"memory\").document_index(...) with file-backed input)"
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
            let output_path = Path::new(&output_path);
            let (temp_output_path, output) = create_atomic_output(output_path, true)?;
            let stream_result: CliResult<()> = (|| {
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
                let output = writer.into_inner();
                output.sync_all()?;
                drop(output);
                std::fs::rename(&temp_output_path, output_path)?;
                Ok(())
            })();
            if let Err(primary_error) = stream_result {
                return Err(cleanup_atomic_output(&temp_output_path, primary_error));
            }
        } else {
            let input = std::fs::read(input_path)?;
            let container = WriterBuilder::new()
                .options(options)
                .profile(&profile)
                .dense_line_index(dense_line_index)
                .pack(&input)?;
            write_container_atomically(&output_path, &container)?;
        }
        Ok(())
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Qzt(QztError::MetadataInvalid)) if profile == "memory" => {
            eprintln!(
                "qzt pack: --profile memory requires a DocumentIndex; use WriterBuilder::profile(\"memory\").document_index(...)"
            );
            ExitCode::from(1)
        }
        Err(error) => command_failed("pack", &error),
    }
}

struct PackDocsArgs {
    input_paths: Vec<String>,
    output_path: String,
    doc_id_prefix: String,
    options: WriterOptions,
    profile: String,
    dense_line_index: Option<bool>,
}

impl PackDocsArgs {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut input_paths = Vec::new();
        let mut output_path = None;
        let mut doc_id_prefix = String::new();
        let mut options = WriterOptions::default();
        let mut profile = String::from("core");
        let mut dense_line_index = None;
        let mut target_chunk_size_configured = false;
        let mut max_chunk_size_configured = false;
        let mut args = args;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-o" | "--output" => {
                    let Some(path) = args.next() else {
                        return Err("missing output path".to_owned());
                    };
                    output_path = Some(path);
                }
                "--doc-id-prefix" => {
                    let Some(prefix) = args.next() else {
                        return Err("missing --doc-id-prefix value".to_owned());
                    };
                    doc_id_prefix = prefix;
                }
                "--profile" => {
                    let Some(value) = args.next() else {
                        return Err("missing --profile value".to_owned());
                    };
                    if !matches!(
                        value.as_str(),
                        "minimal" | "core" | "log" | "archive" | "memory"
                    ) {
                        return Err("invalid --profile value".to_owned());
                    }
                    profile = value;
                }
                "--chunk-size" => {
                    let Some(size) = args.next().and_then(|value| value.parse::<usize>().ok())
                    else {
                        return Err("invalid --chunk-size".to_owned());
                    };
                    options.chunker.target_chunk_size = size;
                    target_chunk_size_configured = true;
                }
                "--max-chunk-size" => {
                    let Some(size) = args.next().and_then(|value| value.parse::<usize>().ok())
                    else {
                        return Err("invalid --max-chunk-size".to_owned());
                    };
                    options.chunker.max_chunk_size = size;
                    max_chunk_size_configured = true;
                }
                "--zstd-level" => {
                    let Some(level) = args.next().and_then(|value| value.parse::<i32>().ok())
                    else {
                        return Err("invalid --zstd-level".to_owned());
                    };
                    options.zstd_level = level;
                }
                "--checksum" => {
                    if args.next().as_deref() != Some("blake3") {
                        return Err("only blake3 checksum is supported".to_owned());
                    }
                }
                "--dict" => {
                    if args.next().as_deref() != Some("none") {
                        return Err("CLI dictionary writing is not implemented".to_owned());
                    }
                }
                "--dense-line-index" => {
                    let Some(value) = args.next() else {
                        return Err("missing --dense-line-index value".to_owned());
                    };
                    if !matches!(value.as_str(), "on" | "off") {
                        return Err("invalid --dense-line-index value".to_owned());
                    }
                    dense_line_index = Some(value == "on");
                }
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown option '{arg}'"));
                }
                _ => input_paths.push(arg),
            }
        }

        if input_paths.is_empty() {
            return Err("missing input file".to_owned());
        }
        let Some(output_path) = output_path else {
            return Err("missing -o output.qzt".to_owned());
        };

        // Memory-profile document extraction decodes whole chunks. A conservative
        // default keeps small document/range reads bounded without changing core's
        // throughput-oriented 4 MiB defaults; explicit chunk sizes remain authoritative.
        if profile == "memory" {
            if !target_chunk_size_configured {
                options.chunker.target_chunk_size = if max_chunk_size_configured {
                    PACK_DOCS_MEMORY_DEFAULT_TARGET_CHUNK_SIZE.min(options.chunker.max_chunk_size)
                } else {
                    PACK_DOCS_MEMORY_DEFAULT_TARGET_CHUNK_SIZE
                };
            }
            if !max_chunk_size_configured {
                options.chunker.max_chunk_size =
                    PACK_DOCS_MEMORY_DEFAULT_MAX_CHUNK_SIZE.max(options.chunker.target_chunk_size);
            }
        }

        let parsed = Self {
            input_paths,
            output_path,
            doc_id_prefix,
            options,
            profile,
            dense_line_index,
        };
        parsed.validate()?;
        Ok(parsed)
    }

    fn validate(&self) -> Result<(), String> {
        self.options
            .chunker
            .validate()
            .map_err(|error| error.to_string())?;
        if self.input_paths.iter().any(|path| path == "-") {
            return Err("stdin is not supported; provide input file paths".to_owned());
        }

        let mut doc_ids = HashSet::with_capacity(self.input_paths.len());
        for input_path in &self.input_paths {
            let Some(doc_id) = pack_docs_document_id(input_path, &self.doc_id_prefix) else {
                return Err("input path has no UTF-8 basename".to_owned());
            };
            if !doc_ids.insert(doc_id.clone()) {
                return Err(format!("duplicate doc_id: {doc_id}"));
            }
        }

        Ok(())
    }
}

fn run_pack_docs(args: impl Iterator<Item = String>) -> ExitCode {
    let args: Vec<String> = args.collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return print_pack_docs_help();
    }

    let args = match PackDocsArgs::parse(args.into_iter()) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("qzt pack-docs: {error}");
            return ExitCode::from(2);
        }
    };

    match pack_docs(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => command_failed("pack-docs", &error),
    }
}

fn pack_docs_document_id(input_path: &str, doc_id_prefix: &str) -> Option<String> {
    let basename = Path::new(input_path).file_name()?.to_str()?;
    Some(format!("{doc_id_prefix}{basename}"))
}

fn pack_docs(args: PackDocsArgs) -> CliResult<()> {
    let (input, spans) = load_pack_docs_input(&args)?;
    let PackDocsArgs {
        output_path,
        options,
        profile,
        dense_line_index,
        ..
    } = args;
    let container = build_pack_docs_container(options, &profile, dense_line_index, &input, spans)?;
    write_container_atomically(&output_path, &container)
}

fn load_pack_docs_input(args: &PackDocsArgs) -> CliResult<(Vec<u8>, Vec<DocumentSpan>)> {
    let mut input = Vec::new();
    let mut spans = Vec::with_capacity(args.input_paths.len());
    for input_path in &args.input_paths {
        let doc_id = pack_docs_document_id(input_path, &args.doc_id_prefix).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "input path has no UTF-8 basename",
            )
        })?;
        let bytes = std::fs::read(input_path)?;
        let offset = u64::try_from(input.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
        let byte_length =
            u64::try_from(bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
        input.extend_from_slice(&bytes);
        spans.push(DocumentSpan::new(doc_id, offset, byte_length));
    }
    Ok((input, spans))
}

fn build_pack_docs_container(
    options: WriterOptions,
    profile: &str,
    dense_line_index: Option<bool>,
    input: &[u8],
    spans: Vec<DocumentSpan>,
) -> CliResult<Vec<u8>> {
    let mut builder = WriterBuilder::new()
        .options(options)
        .profile(profile)
        .document_spans(spans);
    if let Some(enabled) = dense_line_index {
        builder = builder.dense_line_index(enabled);
    }
    Ok(builder.pack(input)?)
}

fn write_container_atomically(output_path: &str, container: &[u8]) -> CliResult<()> {
    let output_path = Path::new(output_path);
    let (temp_output_path, mut file) = create_atomic_output(output_path, false)?;
    let write_result: CliResult<()> = (|| {
        file.write_all(container)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp_output_path, output_path)?;
        Ok(())
    })();
    if let Err(primary_error) = write_result {
        return Err(cleanup_atomic_output(&temp_output_path, primary_error));
    }
    Ok(())
}

fn create_atomic_output(
    output_path: &Path,
    read_access: bool,
) -> CliResult<(std::path::PathBuf, std::fs::File)> {
    let file_name = output_path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "output path has no file name",
        )
    })?;
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let inherited_permissions = match std::fs::symlink_metadata(output_path) {
        Ok(metadata) if metadata.file_type().is_file() => Some(metadata.permissions()),
        Ok(_) => None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    // Why create_new in the destination directory: pack writes can be large, so
    // we stream without buffering the container while preventing a pre-created
    // symlink from redirecting/truncating an unrelated file before atomic rename.
    for attempt in 0_u16..128 {
        let mut name = std::ffi::OsString::from(".");
        name.push(file_name);
        name.push(format!(".qzt-tmp-{}-{attempt}", std::process::id()));
        let path = parent.join(name);
        match std::fs::OpenOptions::new()
            .read(read_access)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => {
                if let Some(permissions) = inherited_permissions {
                    if let Err(primary_error) = file.set_permissions(permissions) {
                        drop(file);
                        return Err(cleanup_atomic_output(&path, primary_error.into()));
                    }
                }
                return Ok((path, file));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a unique temporary output file",
    )
    .into())
}

fn cleanup_atomic_output(temp_output_path: &Path, primary_error: CliError) -> CliError {
    match std::fs::remove_file(temp_output_path) {
        Ok(()) => primary_error,
        Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => primary_error,
        Err(cleanup_error) => std::io::Error::other(format!(
            "{primary_error}; additionally failed to remove temporary output: {cleanup_error}"
        ))
        .into(),
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
                write_stdout_with(|output| {
                    writeln!(output, "Format: QZT 0.1")?;
                    writeln!(output, "Profile: {}", metadata.profile)?;
                    writeln!(output, "Original size: {}", info.original_size)?;
                    writeln!(output, "Compressed size: {compressed_size}")?;
                    writeln!(output, "Chunks: {}", info.chunk_count)?;
                    writeln!(output, "Lines: {}", info.line_count)?;
                    writeln!(output, "Compression: zstd")?;
                    writeln!(output, "Zstd level: {}", metadata.zstd_level)?;
                    writeln!(output, "Target chunk size: {}", metadata.target_chunk_size)?;
                    writeln!(output, "Max chunk size: {}", metadata.max_chunk_size)?;
                    writeln!(output, "Line index: {line_index}")?;
                    writeln!(
                        output,
                        "Document index: {}",
                        if metadata.document_index { "yes" } else { "no" }
                    )?;
                    writeln!(output, "Checksum: blake3")?;
                    writeln!(output, "Zstd stream compatible: no")?;
                    // New lines for container identity and original checksum.
                    writeln!(
                        output,
                        "Container ID: {}",
                        cli_json::hex(&info.container_id)
                    )?;
                    writeln!(
                        output,
                        "Original checksum: {}:{}",
                        metadata.original_checksum.algorithm,
                        cli_json::hex(&metadata.original_checksum.value),
                    )?;
                    writeln!(output, "Newline mode: {}", metadata.newline_mode)
                })
            } else {
                // JSON output: single object on stdout.
                let container_id_hex = cli_json::hex(&info.container_id);
                let checksum_alg = cli_json::escape(&metadata.original_checksum.algorithm);
                let checksum_value = cli_json::hex(&metadata.original_checksum.value);
                let profile = cli_json::escape(&metadata.profile);
                let newline_mode = cli_json::escape(&metadata.newline_mode);
                let output = format!(
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
                        "}}\n"
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
                write_stdout(output.as_bytes())
            }
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
    if let Some(arg) = args.next() {
        eprintln!("qzt range: unknown option '{arg}'");
        return ExitCode::from(2);
    }

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
fn write_search_report_text(report: &SearchReport, output: &mut dyn Write) -> std::io::Result<()> {
    for hit in &report.hits {
        writeln!(
            output,
            "hit logical_offset={} byte_length={} chunk_start={} chunk_end={} source={}",
            hit.logical_offset, hit.byte_length, hit.chunk_start, hit.chunk_end, hit.source
        )?;
    }
    // Escape query so LF/CR/quotes cannot break the single-line metrics contract.
    let query_escaped = cli_json::escape(&report.metrics.query);
    writeln!(
        output,
        "metrics query={} index_kind={} posting_granularity={} index_size_bytes={} source_size_bytes={} index_size_ratio={:.6} term_lookups={} posting_bytes_read={} candidate_granules={} candidate_chunks={} decoded_bytes={} physical_decoded_bytes={} verified_matches={} query_time_ms={:.3} capped={} incomplete_reason={}",
        query_escaped,
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
    )?;
    if let Some(reason) = report.incomplete_reason {
        eprintln!("qzt search: warning: result may be incomplete ({reason})");
    }
    Ok(())
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
fn write_search_report_json(report: &SearchReport, output: &mut dyn Write) -> std::io::Result<()> {
    let query_escaped = cli_json::escape(&report.metrics.query);
    let index_kind_escaped = cli_json::escape(report.metrics.index_kind);
    let granularity_escaped = cli_json::escape(report.metrics.posting_granularity);

    write!(output, "{{\"hits\":[")?;
    for (i, hit) in report.hits.iter().enumerate() {
        if i > 0 {
            write!(output, ",")?;
        }
        let source_escaped = cli_json::escape(hit.source);
        write!(
            output,
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
        )?;
    }
    let incomplete_json = match report.incomplete_reason {
        None => "null".to_owned(),
        Some(reason) => format!("\"{}\"", cli_json::escape(reason)),
    };
    // Guard against NaN/inf producing invalid JSON for the f64 metric fields.
    debug_assert!(report.metrics.index_size_ratio.is_finite());
    debug_assert!(report.metrics.query_time_ms.is_finite());
    writeln!(
        output,
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
    )?;
    if let Some(reason) = report.incomplete_reason {
        eprintln!("qzt search: warning: result may be incomplete ({reason})");
    }
    Ok(())
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

fn run_attest(args: impl Iterator<Item = String>) -> ExitCode {
    let args: Vec<String> = args.collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return print_attest_help();
    }

    let mut args = args.into_iter();
    let mut path = None;
    let mut level = VerifyLevel::Deep;
    while let Some(arg) = args.next() {
        if arg.as_str() == "--level" {
            let Some(value) = args.next() else {
                eprintln!("qzt attest: missing --level value");
                return ExitCode::from(2);
            };
            level = match value.as_str() {
                "quick" => VerifyLevel::Quick,
                "normal" => VerifyLevel::Normal,
                "deep" => VerifyLevel::Deep,
                _ => {
                    eprintln!(
                        "qzt attest: invalid --level value '{value}' (expected quick, normal, or deep)"
                    );
                    return ExitCode::from(2);
                }
            };
        } else if arg.starts_with('-') {
            eprintln!("qzt attest: unknown option '{arg}'");
            return ExitCode::from(2);
        } else {
            if path.is_some() {
                eprintln!("qzt attest: unexpected extra file argument");
                return ExitCode::from(2);
            }
            path = Some(arg);
        }
    }
    let Some(path) = path else {
        eprintln!("qzt attest: missing file");
        return ExitCode::from(2);
    };

    let result: CliResult<String> = (|| {
        let reader = QztFileReader::open_path(path)?;
        // Verification must finish before any stdout write. This prevents a
        // corrupt container from yielding a partial claim that could be signed.
        let verify_report = reader.verify(level)?;
        let info = reader.info();
        let details = reader.skeleton_details();
        Ok(cli_attest::Attestation {
            info: &info,
            original_checksum: &details.metadata.original_checksum,
            container_checksum: details.footer_payload.container_checksum.as_ref(),
            final_file_size: details.footer_payload.final_file_size,
            verify_report: &verify_report,
        }
        .render())
    })();

    match result {
        Ok(attestation) => write_stdout(attestation.as_bytes()),
        Err(error) => command_failed("attest", &error),
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
                write_stdout_with(|output| {
                    writeln!(output, "Verify: {:?} ok", report.level)?;
                    writeln!(output, "Checked chunks: {}", report.checked_chunks)?;
                    writeln!(output, "Decoded bytes: {}", report.decoded_bytes)
                })
            } else {
                let chunks = report.checked_chunks;
                let bytes = report.decoded_bytes;
                let output = format!(
                    "{{\"ok\":true,\"level\":\"{level_str}\",\"checked_chunks\":{chunks},\"decoded_bytes\":{bytes}}}"
                );
                write_stdout(format!("{output}\n").as_bytes())
            }
        }
        Err(ref error) => {
            if format == VerifyFormat::Json {
                // JSON consumers read stdout only; no stderr output in JSON mode.
                let error_msg = cli_json::escape(&error.to_string());
                let level_str = verify_level_as_str(level);
                let output = format!(
                    "{{\"ok\":false,\"level\":\"{level_str}\",\"error\":\"{error_msg}\"}}\n"
                );
                match write_stdout(output.as_bytes()) {
                    code if code == ExitCode::SUCCESS => ExitCode::from(1),
                    code => code,
                }
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
    let mut zero_based = false;
    for arg in args {
        if arg == "--zero-based" {
            zero_based = true;
        } else {
            eprintln!("qzt line: unknown option '{arg}'");
            return ExitCode::from(2);
        }
    }
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
            write_stdout_with(|output| {
                if format == DocsFormat::Text {
                    writeln!(output, "doc_id\toffset\tbytes\tfirst_line\tlines\tchecksum")?;
                    for doc in &documents {
                        let doc_id_escaped = escape_doc_id_text(&doc.doc_id);
                        let checksum_hex = cli_json::hex(&doc.checksum.value);
                        let first_line_one_based = doc.first_line.saturating_add(1);
                        writeln!(
                            output,
                            "{}\t{}\t{}\t{}\t{}\t{}:{}",
                            doc_id_escaped,
                            doc.logical_offset,
                            doc.byte_length,
                            first_line_one_based,
                            doc.line_count,
                            cli_json::escape(&doc.checksum.algorithm),
                            checksum_hex,
                        )?;
                    }
                    Ok(())
                } else {
                    // JSON output: {"documents":[...]}
                    write!(output, "{{\"documents\":[")?;
                    for (i, doc) in documents.iter().enumerate() {
                        if i > 0 {
                            write!(output, ",")?;
                        }
                        let doc_id_json = cli_json::escape(&doc.doc_id);
                        let alg_json = cli_json::escape(&doc.checksum.algorithm);
                        let checksum_hex = cli_json::hex(&doc.checksum.value);
                        let first_line_one_based = doc.first_line.saturating_add(1);
                        write!(
                            output,
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
                        )?;
                    }
                    writeln!(output, "]}}")
                }
            })
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
        Ok(report) => write_stdout_with(|output| match format {
            SearchFormat::Text => write_search_report_text(&report, output),
            SearchFormat::Json => write_search_report_json(&report, output),
        }),
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

#[derive(Debug, Clone, Copy)]
enum InspectSidecarFormat {
    Text,
    Json,
}

fn run_inspect_sidecar(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt inspect-sidecar: missing QZT file");
        return ExitCode::from(2);
    };

    let mut sidecar_path = None;
    let mut format = InspectSidecarFormat::Text;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sidecar" => {
                let Some(path) = args.next() else {
                    eprintln!("qzt inspect-sidecar: missing --sidecar path");
                    return ExitCode::from(2);
                };
                sidecar_path = Some(path);
            }
            "--format" => {
                let Some(value) = args.next() else {
                    eprintln!("qzt inspect-sidecar: missing --format value");
                    return ExitCode::from(2);
                };
                format = match value.as_str() {
                    "text" => InspectSidecarFormat::Text,
                    "json" => InspectSidecarFormat::Json,
                    _ => {
                        eprintln!(
                            "qzt inspect-sidecar: unknown --format value '{value}' (expected text or json)"
                        );
                        return ExitCode::from(2);
                    }
                };
            }
            _ => {
                eprintln!("qzt inspect-sidecar: unknown option '{arg}'");
                return ExitCode::from(2);
            }
        }
    }
    let Some(sidecar_path) = sidecar_path else {
        eprintln!("qzt inspect-sidecar: missing --sidecar file.qzi");
        return ExitCode::from(2);
    };

    let result: CliResult<_> = (|| {
        let reader = QztFileReader::open_path(&path)?;
        let sidecar = QziFileSidecar::open_path(sidecar_path, &reader)?;
        Ok(sidecar)
    })();

    match result {
        Ok(sidecar) => {
            let manifest = sidecar.manifest();
            let ngram_text = manifest
                .ngram_n
                .map_or_else(|| "none".to_owned(), |n| n.to_string());
            write_stdout_with(|output| match format {
                InspectSidecarFormat::Text => writeln!(
                    output,
                    concat!(
                        "index_type={}\n",
                        "ngram_n={}\n",
                        "complete={}\n",
                        "high_df_per_million={}\n",
                        "source_size_bytes={}\n",
                        "index_size_bytes={}\n",
                        "granule_count={}\n",
                        "term_count={}\n",
                        "postings_size_bytes={}"
                    ),
                    manifest.index_type,
                    ngram_text,
                    manifest.complete,
                    manifest.high_df_per_million,
                    manifest.source_size_bytes,
                    manifest.index_size_bytes,
                    sidecar.granule_count(),
                    sidecar.term_count(),
                    sidecar.postings_size_bytes(),
                ),
                InspectSidecarFormat::Json => {
                    let ngram_json = manifest
                        .ngram_n
                        .map_or_else(|| "null".to_owned(), |n| n.to_string());
                    writeln!(
                        output,
                        concat!(
                            "{{\"index_type\":\"{}\",",
                            "\"ngram_n\":{},",
                            "\"complete\":{},",
                            "\"high_df_per_million\":{},",
                            "\"source_size_bytes\":{},",
                            "\"index_size_bytes\":{},",
                            "\"granule_count\":{},",
                            "\"term_count\":{},",
                            "\"postings_size_bytes\":{}}}"
                        ),
                        cli_json::escape(&manifest.index_type),
                        ngram_json,
                        manifest.complete,
                        manifest.high_df_per_million,
                        manifest.source_size_bytes,
                        manifest.index_size_bytes,
                        sidecar.granule_count(),
                        sidecar.term_count(),
                        sidecar.postings_size_bytes(),
                    )
                }
            })
        }
        Err(error) => command_failed("inspect-sidecar", &error),
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

fn read_line_range_file<R: ReadAt>(
    reader: &QztFileReader<R>,
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
    write_stdout_with(|output| output.write_all(bytes))
}

fn write_stdout_with(write: impl FnOnce(&mut dyn Write) -> std::io::Result<()>) -> ExitCode {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    match write(&mut output).and_then(|()| output.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("qzt: failed to write stdout: {error}");
            ExitCode::from(1)
        }
    }
}

fn command_failed(command: &str, error: &CliError) -> ExitCode {
    eprintln!("qzt {command}: {error}");
    ExitCode::from(1)
}
