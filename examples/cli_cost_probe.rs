//! Opt-in file-backed phase probe for the #295 CLI cost measurement.
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use qzt::{
    CorpusKind, QziFileSidecar, QztFileReader, ReadAt, SearchOptions, ValidationCorpusOptions,
    generate_validation_corpus,
};

const MARKER: &[u8] = b"\nissue295-needle-unique\n";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // nosemgrep: rust.lang.security.args-os.args-os -- skip(1) discards argv[0]; take(9) bounds argument count; corpus and sample sizes are capped below.
    let args: Vec<OsString> = env::args_os().skip(1).take(9).collect();
    match args.first().and_then(|value| value.to_str()) {
        Some("generate") if args.len() == 4 => {
            let target = utf8_arg(&args[2])?.parse::<usize>()?;
            let seed = utf8_arg(&args[3])?.parse::<u64>()?;
            if target == 0 || target > 1024 * 1024 * 1024 {
                return Err("target must be within 1..=1073741824 bytes".into());
            }
            let mut corpus = generate_validation_corpus(
                CorpusKind::C2Logs,
                ValidationCorpusOptions { seed, target_bytes: target },
            )?;
            corpus.extend_from_slice(MARKER);
            fs::write(Path::new(&args[1]), &corpus)?;
            // One-off metadata tally does not justify a new crate dependency.
            #[allow(clippy::naive_bytecount)]
            let lines = corpus.iter().filter(|byte| **byte == b'\n').count();
            println!("corpus_bytes={} lines={} blake3={} seed={} kind=C2+marker marker_offset={}",
                corpus.len(), lines,
                blake3::hash(&corpus).to_hex(), seed, corpus.len() - MARKER.len() + 1);
        }
        Some("api") if args.len() == 8 => {
            let started = Instant::now();
            let reader = QztFileReader::open_path(Path::new(&args[1]))?;
            let qzt_open_ms = started.elapsed().as_secs_f64() * 1000.0;
            let started = Instant::now();
            let sidecar = QziFileSidecar::open_path(Path::new(&args[2]), &reader)?;
            let qzi_open_ms = started.elapsed().as_secs_f64() * 1000.0;
            let query = utf8_arg(&args[3])?;
            let max_results = utf8_arg(&args[4])?.parse::<u64>()?;
            let max_candidates = utf8_arg(&args[5])?.parse::<u64>()?;
            let warmup = utf8_arg(&args[6])?.parse::<usize>()?;
            let samples = utf8_arg(&args[7])?.parse::<usize>()?;
            if samples == 0 || samples > 1000 || warmup > 1000 {
                return Err("sample or warmup count outside 1..=1000".into());
            }
            let options = SearchOptions { max_search_results: max_results,
                max_candidate_granules: max_candidates, ..SearchOptions::default() };
            println!("phase qzt_open_ms={qzt_open_ms:.6} qzi_open_ms={qzi_open_ms:.6}");
            for ordinal in 0..warmup + samples {
                let started = Instant::now();
                let report = sidecar.search(&reader, query, options)?;
                let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
                println!("api_sample warmup={} ordinal={} elapsed_ms={elapsed_ms:.6} hits={} capped={} stop={} incomplete={} decoded_bytes={} physical_decoded_bytes={} physical_decoded_chunks={} candidates={} coverage_verified={}",
                    ordinal < warmup, ordinal, report.hits.len(), report.capped,
                    report.stop_reason.unwrap_or("none"), report.incomplete_reason.unwrap_or("none"),
                    report.metrics.decoded_bytes, report.metrics.physical_decoded_bytes,
                    report.metrics.physical_decoded_chunks, report.metrics.candidate_granules,
                    report.index_coverage_verified);
            }
        }
        Some("profile") if args.len() == 4 => {
            let repeats = utf8_arg(&args[3])?.parse::<usize>()?;
            if repeats == 0 || repeats > 1000 {
                return Err("repeats must be within 1..=1000".into());
            }
            let (qzt_source, qzt_len, qzt_reads, qzt_bytes) = CountingFile::open(Path::new(&args[1]))?;
            let reader = QztFileReader::open_read_at(qzt_source, qzt_len)?;
            let (qzi_source, qzi_len, qzi_reads, qzi_bytes) = CountingFile::open(Path::new(&args[2]))?;
            let sidecar = QziFileSidecar::open_read_at(qzi_source, qzi_len, &reader)?;
            println!("profile qzt_reads={} qzt_requested_bytes={} qzi_reads={} qzi_requested_bytes={} terms={} postings_size={} pid={}",
                qzt_reads.load(Ordering::Relaxed), qzt_bytes.load(Ordering::Relaxed),
                qzi_reads.load(Ordering::Relaxed), qzi_bytes.load(Ordering::Relaxed),
                sidecar.term_count(), sidecar.postings_size_bytes(), std::process::id());
            for _ in 0..repeats {
                let reader = QztFileReader::open_path(Path::new(&args[1]))?;
                std::hint::black_box(QziFileSidecar::open_path(Path::new(&args[2]), &reader)?);
            }
        }
        _ => return Err("usage: cli_cost_probe generate PATH TARGET_BYTES SEED | api QZT QZI QUERY MAX_RESULTS MAX_CANDIDATES WARMUP SAMPLES | profile QZT QZI REPEATS".into()),
    }
    Ok(())
}

struct CountingFile {
    file: Mutex<fs::File>,
    reads: Arc<AtomicU64>,
    bytes: Arc<AtomicU64>,
}

impl CountingFile {
    fn open(path: &Path) -> io::Result<(Self, u64, Arc<AtomicU64>, Arc<AtomicU64>)> {
        let file = fs::File::open(path)?;
        let len = file.metadata()?.len();
        let reads = Arc::new(AtomicU64::new(0));
        let bytes = Arc::new(AtomicU64::new(0));
        Ok((
            Self {
                file: Mutex::new(file),
                reads: reads.clone(),
                bytes: bytes.clone(),
            },
            len,
            reads,
            bytes,
        ))
    }
}

impl ReadAt for CountingFile {
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let mut file = self
            .file
            .lock()
            .map_err(|_| io::Error::other("counting file lock poisoned"))?;
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(buf)?;
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(buf.len() as u64, Ordering::Relaxed);
        Ok(())
    }
}

fn utf8_arg(value: &OsString) -> Result<&str, Box<dyn std::error::Error>> {
    value
        .to_str()
        .ok_or_else(|| "argument must be UTF-8".into())
}
