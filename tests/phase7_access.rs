use std::fs;
use std::process::Command;
use std::time::Instant;

use qzt::chunker::ChunkerOptions;
use qzt::error::QztError;
use qzt::reader::QztReader;
use qzt::writer::{pack_bytes_with_container_id, WriterOptions};

fn options(target_chunk_size: usize, max_chunk_size: usize) -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size,
            max_chunk_size,
        },
        zstd_level: 0,
    }
}

fn pack(input: &[u8], target: usize, max: usize) -> Vec<u8> {
    pack_bytes_with_container_id(input, [0x77; 16], options(target, max)).expect("pack should work")
}

#[test]
fn read_range_within_one_chunk_and_across_chunks() {
    let reader = QztReader::open(pack(b"abcdefghij", 4, 4)).expect("reader should open");

    assert_eq!(reader.read_range(1, 2), Ok(b"bc".to_vec()));
    assert_eq!(reader.read_range(3, 5), Ok(b"defgh".to_vec()));
}

#[test]
fn read_range_zero_length_and_overflow_are_handled() {
    let reader = QztReader::open(pack(b"abc", 4, 4)).expect("reader should open");

    assert_eq!(reader.read_range(1, 0), Ok(Vec::new()));
    assert_eq!(
        reader.read_range(u64::MAX, 1),
        Err(QztError::LogicalRangeOutOfBounds)
    );
    assert_eq!(
        reader.read_range(2, 2),
        Err(QztError::LogicalRangeOutOfBounds)
    );
}

#[test]
fn read_text_range_rejects_invalid_utf8_boundary() {
    let input = "あい".as_bytes();
    let reader = QztReader::open(pack(input, 8, 8)).expect("reader should open");

    assert_eq!(
        reader.read_text_range(1, 2),
        Err(QztError::InvalidUtf8Boundary)
    );
    assert_eq!(reader.read_text_range(0, 3), Ok("あ".to_owned()));
}

#[test]
fn read_line_raw_reads_first_last_and_spanning_lines() {
    let input = b"first\nabcdefghijklmnopqrstuvwxyz\nlast";
    let reader = QztReader::open(pack(input, 8, 8)).expect("reader should open");

    assert_eq!(reader.read_line_raw(0), Ok(b"first\n".to_vec()));
    assert_eq!(
        reader.read_line_raw(1),
        Ok(b"abcdefghijklmnopqrstuvwxyz\n".to_vec())
    );
    assert_eq!(reader.read_line_raw(2), Ok(b"last".to_vec()));
    assert_eq!(reader.read_line_raw(3), Err(QztError::LineOutOfRange));
}

#[test]
fn cli_range_and_line_smoke() {
    let input = b"alpha\nbeta\ngamma\n";
    let container = pack(input, 8, 8);
    let path = std::env::temp_dir().join(format!(
        "qzt-phase7-{}-{}.qzt",
        std::process::id(),
        input.len()
    ));
    fs::write(&path, container).expect("fixture should be written");

    let range = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("range")
        .arg(&path)
        .arg("--bytes")
        .arg("6:10")
        .output()
        .expect("qzt range should run");
    assert!(range.status.success());
    assert_eq!(range.stdout, b"beta");

    let line = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("line")
        .arg(&path)
        .arg("2")
        .output()
        .expect("qzt line should run");
    assert!(line.status.success());
    assert_eq!(line.stdout, b"beta\n");

    let _ = fs::remove_file(path);
}

#[test]
fn phase7_intermediate_benchmark_records_nonzero_metrics() {
    let input = vec![b'a'; 64 * 1024];

    let started = Instant::now();
    let container = pack(&input, 16 * 1024, 16 * 1024);
    let pack_elapsed = started.elapsed();

    let reader = QztReader::open(&container).expect("reader should open");

    let started = Instant::now();
    let exported = reader.export_all().expect("export should work");
    let export_elapsed = started.elapsed();

    let started = Instant::now();
    let range = reader.read_range(1024, 4096).expect("range should work");
    let range_elapsed = started.elapsed();

    let line_input = b"line0\nline1\nline2\n";
    let line_reader = QztReader::open(pack(line_input, 8, 8)).expect("line reader should open");
    let started = Instant::now();
    let line = line_reader.read_line_raw(1).expect("line should read");
    let line_elapsed = started.elapsed();

    assert_eq!(exported, input);
    assert_eq!(range.len(), 4096);
    assert_eq!(line, b"line1\n");
    assert!(pack_elapsed.as_nanos() > 0);
    assert!(export_elapsed.as_nanos() > 0);
    assert!(range_elapsed.as_nanos() > 0);
    assert!(line_elapsed.as_nanos() > 0);

    eprintln!(
        "phase7_bench pack_mib_s={:.3} export_mib_s={:.3} range_mib_s={:.3} line_us={:.3}",
        throughput_mib_s(input.len(), pack_elapsed),
        throughput_mib_s(exported.len(), export_elapsed),
        throughput_mib_s(range.len(), range_elapsed),
        line_elapsed.as_secs_f64() * 1_000_000.0
    );
}

fn throughput_mib_s(bytes: usize, elapsed: std::time::Duration) -> f64 {
    bytes as f64 / elapsed.as_secs_f64() / (1024.0 * 1024.0)
}
