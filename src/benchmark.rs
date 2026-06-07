use std::fmt;
use std::time::{Duration, Instant};

use crate::chunker::ChunkerOptions;
use crate::error::{QztError, Result};
use crate::reader::QztReader;
use crate::search::SearchOptions;
use crate::sidecar::{build_search_sidecar, QziSidecar, SidecarIndexKind};
use crate::writer::{pack_bytes_with_container_id, WriterOptions};

/// Reproducible release benchmark configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseBenchmarkOptions {
    pub line_count: usize,
    pub chunk_size: usize,
    pub range_size: u64,
}

impl Default for ReleaseBenchmarkOptions {
    fn default() -> Self {
        Self {
            line_count: 24_000,
            chunk_size: 8 * 1024,
            range_size: 256 * 1024,
        }
    }
}

/// Release hardening benchmark output.
#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseBenchmarkReport {
    pub corpus_bytes: u64,
    pub line_count: usize,
    pub packed_bytes: u64,
    pub exported_bytes: u64,
    pub qzi_token_bytes: u64,
    pub qzi_ngram_bytes: u64,
    pub compression_ratio: f64,
    pub qzi_token_size_ratio: f64,
    pub qzi_ngram_size_ratio: f64,
    pub pack_mib_s: f64,
    pub export_mib_s: f64,
    pub range_mib_s: f64,
    pub rare_token_candidate_granules: u64,
    pub rare_token_candidate_chunks: u64,
    pub rare_token_decoded_bytes: u64,
    pub rare_token_verified_matches: u64,
    pub common_ngram_candidate_granules: u64,
    pub common_ngram_decoded_bytes: u64,
    pub common_ngram_capped: bool,
    pub raw_scan_decoded_bytes: u64,
}

impl fmt::Display for ReleaseBenchmarkReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "release_bench corpus_bytes={} lines={} packed_bytes={} compression_ratio={:.6} qzi_token_bytes={} qzi_token_ratio={:.6} qzi_ngram_bytes={} qzi_ngram_ratio={:.6} pack_mib_s={:.3} export_mib_s={:.3} range_mib_s={:.3} rare_token_candidate_granules={} rare_token_candidate_chunks={} rare_token_decoded_bytes={} rare_token_verified_matches={} common_ngram_candidate_granules={} common_ngram_decoded_bytes={} common_ngram_capped={} raw_scan_decoded_bytes={}",
            self.corpus_bytes,
            self.line_count,
            self.packed_bytes,
            self.compression_ratio,
            self.qzi_token_bytes,
            self.qzi_token_size_ratio,
            self.qzi_ngram_bytes,
            self.qzi_ngram_size_ratio,
            self.pack_mib_s,
            self.export_mib_s,
            self.range_mib_s,
            self.rare_token_candidate_granules,
            self.rare_token_candidate_chunks,
            self.rare_token_decoded_bytes,
            self.rare_token_verified_matches,
            self.common_ngram_candidate_granules,
            self.common_ngram_decoded_bytes,
            self.common_ngram_capped,
            self.raw_scan_decoded_bytes
        )
    }
}

/// Runs a deterministic larger-corpus benchmark smoke.
pub fn run_release_benchmark(options: ReleaseBenchmarkOptions) -> Result<ReleaseBenchmarkReport> {
    if options.line_count == 0 || options.chunk_size == 0 || options.range_size == 0 {
        return Err(QztError::ResourceLimitExceeded);
    }

    let corpus = release_corpus(options.line_count);
    let corpus_bytes = u64::try_from(corpus.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
    let writer_options = WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size: options.chunk_size,
            max_chunk_size: options.chunk_size,
        },
        zstd_level: 0,
    };

    let started = Instant::now();
    let packed = pack_bytes_with_container_id(&corpus, [0xf0; 16], writer_options)?;
    let pack_elapsed = started.elapsed();
    let packed_bytes = u64::try_from(packed.len()).map_err(|_| QztError::ResourceLimitExceeded)?;

    let reader = QztReader::open(&packed)?;
    let started = Instant::now();
    let exported = reader.export_all()?;
    let export_elapsed = started.elapsed();
    if exported != corpus {
        return Err(QztError::ContainerCorrupt);
    }

    let range_offset = corpus_bytes
        .saturating_sub(options.range_size)
        .saturating_div(2);
    let started = Instant::now();
    let range = reader.read_range(range_offset, options.range_size)?;
    let range_elapsed = started.elapsed();
    let range_bytes = u64::try_from(range.len()).map_err(|_| QztError::ResourceLimitExceeded)?;

    let qzi_token = build_search_sidecar(&packed, SidecarIndexKind::Token)?;
    let qzi_ngram = build_search_sidecar(&packed, SidecarIndexKind::Ngram { n: 3 })?;
    let qzi_token_bytes =
        u64::try_from(qzi_token.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
    let qzi_ngram_bytes =
        u64::try_from(qzi_ngram.len()).map_err(|_| QztError::ResourceLimitExceeded)?;

    let token_sidecar = QziSidecar::open(&packed, &qzi_token)?;
    let rare_report =
        token_sidecar.search(&reader, "rare-token-unique", SearchOptions::default())?;

    let ngram_sidecar = QziSidecar::open(&packed, &qzi_ngram)?;
    let common_report = ngram_sidecar.search(
        &reader,
        "aaa",
        SearchOptions {
            max_candidate_granules: 10,
            ..SearchOptions::default()
        },
    )?;

    Ok(ReleaseBenchmarkReport {
        corpus_bytes,
        line_count: options.line_count,
        packed_bytes,
        exported_bytes: u64::try_from(exported.len())
            .map_err(|_| QztError::ResourceLimitExceeded)?,
        qzi_token_bytes,
        qzi_ngram_bytes,
        compression_ratio: ratio(packed_bytes, corpus_bytes),
        qzi_token_size_ratio: ratio(qzi_token_bytes, corpus_bytes),
        qzi_ngram_size_ratio: ratio(qzi_ngram_bytes, corpus_bytes),
        pack_mib_s: mib_s(corpus_bytes, pack_elapsed),
        export_mib_s: mib_s(corpus_bytes, export_elapsed),
        range_mib_s: mib_s(range_bytes, range_elapsed),
        rare_token_candidate_granules: rare_report.metrics.candidate_granules,
        rare_token_candidate_chunks: rare_report.metrics.candidate_chunks,
        rare_token_decoded_bytes: rare_report.metrics.decoded_bytes,
        rare_token_verified_matches: rare_report.metrics.verified_matches,
        common_ngram_candidate_granules: common_report.metrics.candidate_granules,
        common_ngram_decoded_bytes: common_report.metrics.decoded_bytes,
        common_ngram_capped: common_report.capped,
        raw_scan_decoded_bytes: corpus_bytes,
    })
}

fn release_corpus(line_count: usize) -> Vec<u8> {
    let rare_line = line_count / 2;
    let mut corpus = Vec::with_capacity(line_count.saturating_mul(96));
    for index in 0..line_count {
        if index == rare_line {
            corpus.extend_from_slice(
                format!(
                    "aaa ts={index:06} level=error service=qzt rare-token-unique message=needle sidecar proof line={index:06}\n"
                )
                .as_bytes(),
            );
        } else {
            corpus.extend_from_slice(
                format!(
                    "aaa ts={index:06} level=info service=qzt component=release message=repeated benchmark corpus line={index:06}\n"
                )
                .as_bytes(),
            );
        }
    }
    corpus
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn mib_s(bytes: u64, elapsed: Duration) -> f64 {
    let seconds = elapsed.as_secs_f64().max(1e-9);
    (bytes as f64 / (1024.0 * 1024.0)) / seconds
}
