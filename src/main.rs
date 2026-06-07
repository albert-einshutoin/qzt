use std::io::Write;
use std::process::ExitCode;

use qzt::reader::QztReader;

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
        Some("range") => run_range(args),
        Some("line") => run_line(args),
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
    println!("  range      Print original bytes in a half-open byte range");
    println!("  line       Print one original line");
    println!();
    println!("Options:");
    println!("  -h, --help     Show this help");
    println!("  -V, --version  Show version");
}

fn run_range(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(path) = args.next() else {
        eprintln!("qzt range: missing file");
        return ExitCode::from(2);
    };
    let Some(flag) = args.next() else {
        eprintln!("qzt range: missing --bytes A:B");
        return ExitCode::from(2);
    };
    if flag != "--bytes" {
        eprintln!("qzt range: expected --bytes A:B");
        return ExitCode::from(2);
    }
    let Some(range) = args.next() else {
        eprintln!("qzt range: missing range");
        return ExitCode::from(2);
    };
    let Some((start, end)) = parse_range(&range) else {
        eprintln!("qzt range: invalid range");
        return ExitCode::from(2);
    };

    let result = std::fs::read(path)
        .map_err(|_| ())
        .and_then(|bytes| QztReader::open(bytes).map_err(|_| ()))
        .and_then(|reader| {
            reader
                .read_range(start, end.saturating_sub(start))
                .map_err(|_| ())
        });

    match result {
        Ok(bytes) => write_stdout(&bytes),
        Err(()) => {
            eprintln!("qzt range: failed");
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

fn parse_range(range: &str) -> Option<(u64, u64)> {
    let (start, end) = range.split_once(':')?;
    let start = start.parse().ok()?;
    let end = end.parse().ok()?;
    (start <= end).then_some((start, end))
}

fn write_stdout(bytes: &[u8]) -> ExitCode {
    match std::io::stdout().write_all(bytes) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(1),
    }
}
