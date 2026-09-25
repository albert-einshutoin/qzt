//! Opt-in file-backed phase probe for the #295 CLI cost measurement.
use std::env;
use std::fs;
use std::path::Path;
use std::time::Instant;

use qzt::{
    CorpusKind, QziFileSidecar, QztFileReader, SearchOptions, ValidationCorpusOptions,
    generate_validation_corpus,
};

const MARKER: &[u8] = b"\nissue295-needle-unique\n";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("generate") if args.len() == 4 => {
            let target = args[2].parse::<usize>()?;
            let seed = args[3].parse::<u64>()?;
            if target == 0 || target > 1024 * 1024 * 1024 {
                return Err("target must be within 1..=1073741824 bytes".into());
            }
            let mut corpus = generate_validation_corpus(
                CorpusKind::C2Logs,
                ValidationCorpusOptions { seed, target_bytes: target },
            )?;
            corpus.extend_from_slice(MARKER);
            fs::write(&args[1], &corpus)?;
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
            let query = &args[3];
            let max_results = args[4].parse::<u64>()?;
            let max_candidates = args[5].parse::<u64>()?;
            let warmup = args[6].parse::<usize>()?;
            let samples = args[7].parse::<usize>()?;
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
        _ => return Err("usage: cli_cost_probe generate PATH TARGET_BYTES SEED | api QZT QZI QUERY MAX_RESULTS MAX_CANDIDATES WARMUP SAMPLES".into()),
    }
    Ok(())
}
