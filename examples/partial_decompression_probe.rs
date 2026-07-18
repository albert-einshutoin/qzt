use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Instant;

use qzt::QztFileReader;

const MAX_PROBE_RANGE_BYTES: u64 = 64 * 1024 * 1024;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Preserve filesystem arguments as OS strings: benchmark paths need not be
    // UTF-8, and argv[0] is deliberately ignored rather than trusted.
    let mut args = env::args_os().skip(1);
    let source_path: PathBuf = args.next().ok_or("missing original source path")?.into();
    let qzt_path: PathBuf = args.next().ok_or("missing QZT container path")?.into();
    let offset = parse_u64(args.next(), "offset")?;
    let length = parse_u64(args.next(), "length")?;
    if args.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    if length == 0 || length > MAX_PROBE_RANGE_BYTES {
        return Err("length must be between 1 and 67108864 bytes".into());
    }

    let source_bytes = std::fs::metadata(&source_path)?.len();
    let qzt_bytes = std::fs::metadata(&qzt_path)?.len();
    let reader = QztFileReader::open_path(&qzt_path)?;

    let started = Instant::now();
    let report = reader.read_range_with_metrics(offset, length)?;
    let elapsed = started.elapsed();

    // Read only the same bounded source slice for the correctness oracle. A
    // whole-file comparison would erase the memory property this probe exists
    // to measure.
    let mut expected = vec![0_u8; usize::try_from(length)?];
    let mut source = File::open(source_path)?;
    source.seek(SeekFrom::Start(offset))?;
    source.read_exact(&mut expected)?;
    if report.bytes != expected {
        return Err("restored range differs from original source".into());
    }

    println!(
        "partial_decompression_probe source_bytes={source_bytes} qzt_bytes={qzt_bytes} range_offset={offset} returned_bytes={} decoded_chunks={} decoded_bytes={} compressed_bytes={} range_micros={}",
        report.bytes.len(),
        report.metrics.decoded_chunks,
        report.metrics.decoded_bytes,
        report.metrics.compressed_bytes,
        elapsed.as_micros()
    );
    Ok(())
}

fn parse_u64(value: Option<OsString>, name: &str) -> Result<u64, Box<dyn std::error::Error>> {
    value
        .ok_or_else(|| format!("missing {name}"))?
        .to_str()
        .ok_or_else(|| format!("{name} must be UTF-8"))?
        .parse::<u64>()
        .map_err(Into::into)
}
