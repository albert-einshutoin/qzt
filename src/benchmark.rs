use std::fmt;
use std::time::{Duration, Instant};

use crate::chunker::ChunkerOptions;
use crate::corpus::{generate_validation_corpus, CorpusKind, ValidationCorpusOptions};
use crate::error::{QztError, Result};
use crate::primitives::{u64_to_usize, usize_to_u64};
use crate::reader::{QztFileReader, QztReader};
use crate::search::{RawTokenIndex, SearchOptions, TokenIndexBuildOptions};
use crate::sidecar::{build_search_sidecar, QziFileSidecar, SidecarIndexKind};
use crate::writer::{pack_bytes_with_container_id, WriterOptions};

/// Reproducible release benchmark configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseBenchmarkOptions {
    /// Number of deterministic log lines generated for the benchmark corpus.
    pub line_count: usize,
    /// Target uncompressed QZT chunk size in bytes.
    pub chunk_size: usize,
    /// Logical byte length restored by the range-access benchmark.
    pub range_size: u64,
    /// Number of measured executions for each search query.
    pub query_repetitions: usize,
    /// Number of unmeasured warm-up executions before each query measurement.
    pub query_warmup_repetitions: usize,
}

impl Default for ReleaseBenchmarkOptions {
    fn default() -> Self {
        Self {
            line_count: 24_000,
            chunk_size: 8 * 1024,
            range_size: 256 * 1024,
            query_repetitions: 5,
            query_warmup_repetitions: 2,
        }
    }
}

/// Release hardening benchmark output.
#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseBenchmarkReport {
    /// Original deterministic corpus size in bytes.
    pub corpus_bytes: u64,
    /// Number of lines generated in the corpus.
    pub line_count: usize,
    /// Encoded QZT container size in bytes.
    pub packed_bytes: u64,
    /// Number of bytes restored by a full export.
    pub exported_bytes: u64,
    /// Encoded token QZI sidecar size in bytes.
    pub qzi_token_bytes: u64,
    /// Encoded n-gram QZI sidecar size in bytes.
    pub qzi_ngram_bytes: u64,
    /// `packed_bytes / corpus_bytes`.
    pub compression_ratio: f64,
    /// `qzi_token_bytes / corpus_bytes`.
    pub qzi_token_size_ratio: f64,
    /// `qzi_ngram_bytes / corpus_bytes`.
    pub qzi_ngram_size_ratio: f64,
    /// Observed packing throughput in mebibytes per second.
    pub pack_mib_s: f64,
    /// Observed full-export throughput in mebibytes per second.
    pub export_mib_s: f64,
    /// Observed logical-range restoration throughput in mebibytes per second.
    pub range_mib_s: f64,
    /// Granules selected by the rare-token query planner.
    pub rare_token_candidate_granules: u64,
    /// Chunks decoded while verifying the rare-token query.
    pub rare_token_candidate_chunks: u64,
    /// Original bytes decoded while verifying the rare-token query.
    pub rare_token_decoded_bytes: u64,
    /// Original-byte matches verified for the rare-token query.
    pub rare_token_verified_matches: u64,
    /// Granules selected by the common n-gram query planner.
    pub common_ngram_candidate_granules: u64,
    /// Original bytes decoded while verifying the common n-gram query.
    pub common_ngram_decoded_bytes: u64,
    /// Whether the common n-gram query reached a configured result or work cap.
    pub common_ngram_capped: bool,
    /// Corpus bytes a full raw scan would decode for comparison.
    pub raw_scan_decoded_bytes: u64,
    /// Number of measured executions used for each query latency sample.
    pub query_repetitions: usize,
    /// Number of unmeasured warm-up executions used for each query.
    pub query_warmup_repetitions: usize,
    /// Detailed latency and work metrics for the rare-token query.
    pub rare_token_query: ReleaseBenchmarkQueryReport,
    /// Detailed latency and work metrics for the absent-token query.
    pub missing_token_query: ReleaseBenchmarkQueryReport,
    /// Detailed latency and work metrics for the common n-gram query.
    pub common_ngram_query: ReleaseBenchmarkQueryReport,
}

/// Competitive benchmark smoke configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompetitiveBenchmarkOptions {
    /// Deterministic validation-corpus family to generate.
    pub corpus_kind: CorpusKind,
    /// Requested original corpus size in bytes.
    pub corpus_bytes: usize,
    /// Target uncompressed QZT chunk size in bytes.
    pub chunk_size: usize,
    /// Zero-based logical byte offset used for range comparison.
    pub range_offset: u64,
    /// Logical byte length used for range comparison.
    pub range_size: u64,
}

/// Scalable partial-decompression benchmark configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartialDecompressionBenchmarkOptions {
    /// Deterministic validation-corpus family to generate.
    pub corpus_kind: CorpusKind,
    /// Requested original corpus size in bytes.
    pub corpus_bytes: usize,
    /// Target uncompressed QZT chunk size in bytes.
    pub chunk_size: usize,
    /// Zero-based logical byte offset to restore.
    pub range_offset: u64,
    /// Logical byte length to restore.
    pub range_size: u64,
}

/// Machine-readable evidence from a partial-decompression benchmark run.
#[derive(Debug, Clone, PartialEq)]
pub struct PartialDecompressionBenchmarkReport {
    /// Stable identifier of the generated validation corpus.
    pub corpus_id: &'static str,
    /// Actual original corpus size in bytes.
    pub corpus_bytes: u64,
    /// Encoded QZT container size in bytes.
    pub qzt_bytes: u64,
    /// Logical offset selected after clamping to the corpus boundary.
    pub range_offset: u64,
    /// Exact original bytes returned to the caller.
    pub returned_bytes: u64,
    /// Number of independently compressed chunks decoded.
    pub decoded_chunks: u64,
    /// Total original bytes decoded from intersecting chunks.
    pub decoded_bytes: u64,
    /// Total compressed chunk payload bytes consumed.
    pub compressed_bytes: u64,
    /// Observed range restoration duration in microseconds.
    pub range_micros: u128,
}

impl fmt::Display for PartialDecompressionBenchmarkReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "partial_decompression_benchmark corpus_id={} corpus_bytes={} qzt_bytes={} range_offset={} returned_bytes={} decoded_chunks={} decoded_bytes={} compressed_bytes={} range_micros={}",
            self.corpus_id,
            self.corpus_bytes,
            self.qzt_bytes,
            self.range_offset,
            self.returned_bytes,
            self.decoded_chunks,
            self.decoded_bytes,
            self.compressed_bytes,
            self.range_micros
        )
    }
}

impl Default for CompetitiveBenchmarkOptions {
    fn default() -> Self {
        Self {
            corpus_kind: CorpusKind::C2Logs,
            corpus_bytes: 128 * 1024,
            chunk_size: 8 * 1024,
            range_offset: 16 * 1024,
            range_size: 4 * 1024,
        }
    }
}

/// Competitive benchmark smoke output.
#[derive(Debug, Clone, PartialEq)]
pub struct CompetitiveBenchmarkReport {
    /// Stable identifier of the generated validation corpus.
    pub corpus_id: &'static str,
    /// Original generated corpus size in bytes.
    pub corpus_bytes: u64,
    /// Encoded QZT container size in bytes.
    pub qzt_bytes: u64,
    /// Size of a whole-stream zstd reference encoding in bytes.
    pub raw_zstd_bytes: u64,
    /// Number of original bytes returned by the QZT range read.
    pub qzt_range_bytes: u64,
    /// Number of independently compressed QZT chunks decoded for the range.
    pub qzt_range_decoded_chunks: u64,
    /// Number of original QZT bytes decoded to restore the range.
    pub qzt_range_decoded_bytes: u64,
    /// Number of compressed QZT chunk payload bytes consumed for the range.
    pub qzt_range_compressed_bytes: u64,
    /// Original bytes whole-stream zstd decoded to restore the same range.
    pub raw_zstd_decoded_bytes: u64,
    /// Observed QZT range-read duration in microseconds.
    pub qzt_range_micros: u128,
    /// Observed whole-stream zstd range-read duration in microseconds.
    pub raw_zstd_range_micros: u128,
    /// Verified matches returned by QZT token search.
    pub token_hit_count: u64,
    /// Matches returned by the built-in raw-byte reference scan.
    pub reference_hit_count: u64,
    /// Whether optional external search-tool comparisons were requested.
    pub external_search_tools_enabled: bool,
    /// `ripgrep` match count, or `None` when the optional tool was not run.
    pub ripgrep_hit_count: Option<u64>,
    /// `SQLite FTS5` match count, or `None` when the optional tool was not run.
    pub sqlite_fts5_hit_count: Option<u64>,
}

/// Query-level release benchmark telemetry.
#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseBenchmarkQueryReport {
    pub name: &'static str,
    pub query: &'static str,
    pub iterations: usize,
    pub warmup_iterations: usize,
    pub candidate_granules: u64,
    pub candidate_chunks: u64,
    pub decoded_bytes: u64,
    pub verified_matches: u64,
    pub capped: bool,
    pub p50_query_time_micros: u128,
    pub p95_query_time_micros: u128,
    pub p99_query_time_micros: u128,
}

impl fmt::Display for ReleaseBenchmarkQueryReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} query={} iterations={} warmup={} candidate_granules={} candidate_chunks={} decoded_bytes={} verified_matches={} capped={} p50_us={} p95_us={} p99_us={}",
            self.name,
            self.query,
            self.iterations,
            self.warmup_iterations,
            self.candidate_granules,
            self.candidate_chunks,
            self.decoded_bytes,
            self.verified_matches,
            self.capped,
            self.p50_query_time_micros,
            self.p95_query_time_micros,
            self.p99_query_time_micros
        )
    }
}

impl fmt::Display for ReleaseBenchmarkReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "release_bench corpus_bytes={} lines={} packed_bytes={} compression_ratio={:.6} qzi_token_bytes={} qzi_token_ratio={:.6} qzi_ngram_bytes={} qzi_ngram_ratio={:.6} pack_mib_s={:.3} export_mib_s={:.3} range_mib_s={:.3} rare_token_candidate_granules={} rare_token_candidate_chunks={} rare_token_decoded_bytes={} rare_token_verified_matches={} common_ngram_candidate_granules={} common_ngram_decoded_bytes={} common_ngram_capped={} raw_scan_decoded_bytes={} query_repetitions={} query_warmup_repetitions={} rare_token_query=\"{}\" missing_token_query=\"{}\" common_ngram_query=\"{}\"",
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
            self.raw_scan_decoded_bytes,
            self.query_repetitions,
            self.query_warmup_repetitions,
            self.rare_token_query,
            self.missing_token_query,
            self.common_ngram_query
        )
    }
}

/// Runs a deterministic larger-corpus benchmark smoke.
pub fn run_release_benchmark(options: ReleaseBenchmarkOptions) -> Result<ReleaseBenchmarkReport> {
    if options.line_count == 0
        || options.chunk_size == 0
        || options.range_size == 0
        || options.query_repetitions == 0
    {
        return Err(QztError::ResourceLimitExceeded);
    }

    let corpus = release_corpus(options.line_count);
    run_release_benchmark_with_corpus(&corpus, options)
}

/// Runs a benchmark smoke over a caller-provided corpus.
pub fn run_release_benchmark_with_corpus(
    corpus: &[u8],
    mut options: ReleaseBenchmarkOptions,
) -> Result<ReleaseBenchmarkReport> {
    if corpus.is_empty()
        || options.chunk_size == 0
        || options.range_size == 0
        || options.query_repetitions == 0
    {
        return Err(QztError::ResourceLimitExceeded);
    }

    let corpus_bytes = usize_to_u64(corpus.len())?;
    if options.line_count == 0 {
        options.line_count = line_count_from_corpus(corpus);
    }

    if options.line_count == 0 {
        return Err(QztError::ResourceLimitExceeded);
    }

    let writer_options = WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size: options.chunk_size,
            max_chunk_size: options.chunk_size,
        },
        zstd_level: 0,
    };

    let started = Instant::now();
    let packed = pack_bytes_with_container_id(corpus, [0xf0; 16], writer_options)?;
    let pack_elapsed = started.elapsed();
    let packed_bytes = usize_to_u64(packed.len())?;

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
    let range_bytes = usize_to_u64(range.len())?;

    let qzi_token = build_search_sidecar(&packed, SidecarIndexKind::Token)?;
    let qzi_ngram = build_search_sidecar(&packed, SidecarIndexKind::Ngram { n: 3 })?;
    let qzi_token_bytes = usize_to_u64(qzi_token.len())?;
    let qzi_ngram_bytes = usize_to_u64(qzi_ngram.len())?;

    // Release measurements exercise the same lazy, bounded-memory sidecar path
    // as the CLI. Decoding every posting at open would benchmark the legacy
    // in-memory compatibility API and reject otherwise valid large corpora.
    let file_reader = QztFileReader::open_read_at(packed.as_slice(), packed.len() as u64)?;
    let token_sidecar = QziFileSidecar::open_read_at(
        qzi_token.as_slice(),
        qzi_token.len() as u64,
        &file_reader,
    )?;
    let rare_token_query = run_query_case(
        |query, options| token_sidecar.search(&file_reader, query, options),
        ReleaseBenchmarkQuery {
            name: "rare-token",
            query: "rare-token-unique",
            search_options: SearchOptions::default(),
        },
        options.query_warmup_repetitions,
        options.query_repetitions,
    )?;

    let missing_token_query = run_query_case(
        |query, options| token_sidecar.search(&file_reader, query, options),
        ReleaseBenchmarkQuery {
            name: "missing-token",
            query: "missing-token-for-release-benchmark",
            search_options: SearchOptions::default(),
        },
        options.query_warmup_repetitions,
        options.query_repetitions,
    )?;

    let ngram_sidecar = QziFileSidecar::open_read_at(
        qzi_ngram.as_slice(),
        qzi_ngram.len() as u64,
        &file_reader,
    )?;
    let common_ngram_query = run_query_case(
        |query, options| ngram_sidecar.search(&file_reader, query, options),
        ReleaseBenchmarkQuery {
            name: "common-ngram",
            query: "aaa",
            search_options: SearchOptions {
                max_candidate_granules: 10,
                ..SearchOptions::default()
            },
        },
        options.query_warmup_repetitions,
        options.query_repetitions,
    )?;

    Ok(ReleaseBenchmarkReport {
        corpus_bytes,
        line_count: options.line_count,
        packed_bytes,
        exported_bytes: usize_to_u64(exported.len())?,
        qzi_token_bytes,
        qzi_ngram_bytes,
        compression_ratio: ratio(packed_bytes, corpus_bytes),
        qzi_token_size_ratio: ratio(qzi_token_bytes, corpus_bytes),
        qzi_ngram_size_ratio: ratio(qzi_ngram_bytes, corpus_bytes),
        pack_mib_s: mib_s(corpus_bytes, pack_elapsed),
        export_mib_s: mib_s(corpus_bytes, export_elapsed),
        range_mib_s: mib_s(range_bytes, range_elapsed),
        rare_token_candidate_granules: rare_token_query.candidate_granules,
        rare_token_candidate_chunks: rare_token_query.candidate_chunks,
        rare_token_decoded_bytes: rare_token_query.decoded_bytes,
        rare_token_verified_matches: rare_token_query.verified_matches,
        common_ngram_candidate_granules: common_ngram_query.candidate_granules,
        common_ngram_decoded_bytes: common_ngram_query.decoded_bytes,
        common_ngram_capped: common_ngram_query.capped,
        raw_scan_decoded_bytes: corpus_bytes,
        query_repetitions: options.query_repetitions,
        query_warmup_repetitions: options.query_warmup_repetitions,
        rare_token_query,
        missing_token_query,
        common_ngram_query,
    })
}

/// Runs a deterministic competitive benchmark smoke.
pub fn run_competitive_benchmark(
    options: CompetitiveBenchmarkOptions,
) -> Result<CompetitiveBenchmarkReport> {
    if options.corpus_bytes == 0 || options.chunk_size == 0 || options.range_size == 0 {
        return Err(QztError::ResourceLimitExceeded);
    }

    let corpus = generate_validation_corpus(
        options.corpus_kind,
        ValidationCorpusOptions {
            seed: 0x18,
            target_bytes: options.corpus_bytes,
        },
    )?;
    let corpus_bytes = usize_to_u64(corpus.len())?;
    let writer_options = WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size: options.chunk_size,
            max_chunk_size: options.chunk_size,
        },
        zstd_level: 0,
    };
    let qzt = pack_bytes_with_container_id(&corpus, [0x18; 16], writer_options)?;
    let raw_zstd =
        zstd::stream::encode_all(corpus.as_slice(), 0).map_err(|_| QztError::ZstdEncodeError)?;

    let offset = options
        .range_offset
        .min(corpus_bytes.saturating_sub(options.range_size));
    let length = options.range_size.min(corpus_bytes - offset);
    let offset_start_us = u64_to_usize(offset)?;
    let expected = &corpus[offset_start_us..(offset_start_us + u64_to_usize(length)?)];

    let qzt_reader = QztFileReader::open_read_at(&qzt[..], qzt.len() as u64)?;
    let started = Instant::now();
    let qzt_range = qzt_reader.read_range_with_metrics(offset, length)?;
    let qzt_range_elapsed = started.elapsed();
    if qzt_range.bytes != expected {
        return Err(QztError::ContainerCorrupt);
    }

    let started = Instant::now();
    let raw_decoded =
        zstd::stream::decode_all(raw_zstd.as_slice()).map_err(|_| QztError::ZstdDecodeError)?;
    let raw_elapsed = started.elapsed();
    let offset_us = u64_to_usize(offset)?;
    let end_us = u64_to_usize(offset + length)?;
    if raw_decoded[offset_us..end_us] != *expected {
        return Err(QztError::ContainerCorrupt);
    }

    let memory_reader = QztReader::open(&qzt)?;
    let token_index = RawTokenIndex::build_from_container(&qzt, TokenIndexBuildOptions::default())?;
    let token_report = token_index.search(&memory_reader, "qzt", SearchOptions::default())?;
    let reference_hit_count = count_substring(&corpus, b"qzt")?;
    let external_report = run_external_search_tools(&corpus, reference_hit_count)?;

    Ok(CompetitiveBenchmarkReport {
        corpus_id: options.corpus_kind.id(),
        corpus_bytes,
        qzt_bytes: usize_to_u64(qzt.len())?,
        raw_zstd_bytes: usize_to_u64(raw_zstd.len())?,
        qzt_range_bytes: usize_to_u64(qzt_range.bytes.len())?,
        qzt_range_decoded_chunks: qzt_range.metrics.decoded_chunks,
        qzt_range_decoded_bytes: qzt_range.metrics.decoded_bytes,
        qzt_range_compressed_bytes: qzt_range.metrics.compressed_bytes,
        raw_zstd_decoded_bytes: usize_to_u64(raw_decoded.len())?,
        qzt_range_micros: qzt_range_elapsed.as_micros(),
        raw_zstd_range_micros: raw_elapsed.as_micros(),
        token_hit_count: token_report.metrics.verified_matches,
        reference_hit_count,
        external_search_tools_enabled: cfg!(feature = "bench-compete"),
        ripgrep_hit_count: external_report.ripgrep_hit_count,
        sqlite_fts5_hit_count: external_report.sqlite_fts5_hit_count,
    })
}

/// Runs a deterministic range-read benchmark without whole-file decode or search work.
///
/// # Errors
///
/// Returns [`QztError::ResourceLimitExceeded`] for a zero corpus, chunk, or
/// range size, plus generation, packing, opening, and range-read errors.
pub fn run_partial_decompression_benchmark(
    options: PartialDecompressionBenchmarkOptions,
) -> Result<PartialDecompressionBenchmarkReport> {
    if options.corpus_bytes == 0 || options.chunk_size == 0 || options.range_size == 0 {
        return Err(QztError::ResourceLimitExceeded);
    }

    let corpus = generate_validation_corpus(
        options.corpus_kind,
        ValidationCorpusOptions {
            seed: 0x46,
            target_bytes: options.corpus_bytes,
        },
    )?;
    let corpus_bytes = usize_to_u64(corpus.len())?;
    if options.range_size > corpus_bytes {
        return Err(QztError::LogicalRangeOutOfBounds);
    }
    let offset = options
        .range_offset
        .min(corpus_bytes.saturating_sub(options.range_size));
    let writer_options = WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size: options.chunk_size,
            max_chunk_size: options.chunk_size,
        },
        zstd_level: 0,
    };
    let qzt = pack_bytes_with_container_id(&corpus, [0x46; 16], writer_options)?;
    let qzt_bytes = usize_to_u64(qzt.len())?;
    let reader = QztFileReader::open_read_at(qzt.as_slice(), qzt_bytes)?;

    let started = Instant::now();
    let range = reader.read_range_with_metrics(offset, options.range_size)?;
    let elapsed = started.elapsed();
    let start = u64_to_usize(offset)?;
    let end = u64_to_usize(offset + options.range_size)?;
    if range.bytes != corpus[start..end] {
        return Err(QztError::ContainerCorrupt);
    }

    Ok(PartialDecompressionBenchmarkReport {
        corpus_id: options.corpus_kind.id(),
        corpus_bytes,
        qzt_bytes,
        range_offset: offset,
        returned_bytes: usize_to_u64(range.bytes.len())?,
        decoded_chunks: range.metrics.decoded_chunks,
        decoded_bytes: range.metrics.decoded_bytes,
        compressed_bytes: range.metrics.compressed_bytes,
        range_micros: elapsed.as_micros(),
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

fn line_count_from_corpus(corpus: &[u8]) -> usize {
    if corpus.is_empty() {
        return 0;
    }

    #[allow(clippy::naive_bytecount)]
    let newline_count = corpus.iter().filter(|byte| **byte == b'\n').count();
    if corpus.ends_with(b"\n") {
        newline_count
    } else {
        newline_count + 1
    }
}

#[derive(Debug, Clone, Copy)]
struct ReleaseBenchmarkQuery {
    name: &'static str,
    query: &'static str,
    search_options: SearchOptions,
}

fn run_query_case(
    mut search: impl FnMut(&str, SearchOptions) -> Result<crate::search::SearchReport>,
    query: ReleaseBenchmarkQuery,
    warmup_repetitions: usize,
    query_repetitions: usize,
) -> Result<ReleaseBenchmarkQueryReport> {
    for _ in 0..warmup_repetitions {
        let _ = search(query.query, query.search_options)?;
    }

    let mut samples = Vec::with_capacity(query_repetitions);
    let mut baseline = None;
    for _ in 0..query_repetitions {
        let started = Instant::now();
        let current = search(query.query, query.search_options)?;
        samples.push(started.elapsed().as_micros());
        let current = ReleaseBenchmarkQueryReportBaseline {
            candidate_granules: current.metrics.candidate_granules,
            candidate_chunks: current.metrics.candidate_chunks,
            decoded_bytes: current.metrics.decoded_bytes,
            verified_matches: current.metrics.verified_matches,
            capped: current.capped,
        };
        if let Some(previous) = baseline {
            if current != previous {
                return Err(QztError::BenchmarkMetricsMismatch);
            }
        } else {
            baseline = Some(current);
        }
    }

    let Some(baseline) = baseline else {
        return Err(QztError::ResourceLimitExceeded);
    };
    samples.sort_unstable();
    let p50_query_time_micros = percentile_micros(&samples, 50);
    let p95_query_time_micros = percentile_micros(&samples, 95);
    let p99_query_time_micros = percentile_micros(&samples, 99);

    Ok(ReleaseBenchmarkQueryReport {
        name: query.name,
        query: query.query,
        iterations: query_repetitions,
        warmup_iterations: warmup_repetitions,
        candidate_granules: baseline.candidate_granules,
        candidate_chunks: baseline.candidate_chunks,
        decoded_bytes: baseline.decoded_bytes,
        verified_matches: baseline.verified_matches,
        capped: baseline.capped,
        p50_query_time_micros,
        p95_query_time_micros,
        p99_query_time_micros,
    })
}

#[allow(clippy::cast_possible_truncation)] // clamped_percentile <= 100, fits in usize
fn percentile_micros(samples: &[u128], percentile: u64) -> u128 {
    if samples.is_empty() {
        return 0;
    }

    debug_assert!(percentile <= 100);
    let clamped_percentile = if percentile > 100 { 100 } else { percentile };
    let rank = (clamped_percentile as usize * samples.len()).div_ceil(100);
    let index = rank.saturating_sub(1).min(samples.len() - 1);
    samples[index]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReleaseBenchmarkQueryReportBaseline {
    candidate_granules: u64,
    candidate_chunks: u64,
    decoded_bytes: u64,
    verified_matches: u64,
    capped: bool,
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

fn count_substring(haystack: &[u8], needle: &[u8]) -> Result<u64> {
    if needle.is_empty() {
        return Ok(0);
    }
    let count = haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count();
    usize_to_u64(count)
}

#[derive(Debug, Clone, Copy)]
struct ExternalToolReport {
    ripgrep_hit_count: Option<u64>,
    sqlite_fts5_hit_count: Option<u64>,
}

#[cfg(not(feature = "bench-compete"))]
fn run_external_search_tools(_corpus: &[u8], _expected_hits: u64) -> Result<ExternalToolReport> {
    Ok(ExternalToolReport {
        ripgrep_hit_count: None,
        sqlite_fts5_hit_count: None,
    })
}

#[cfg(feature = "bench-compete")]
fn run_external_search_tools(corpus: &[u8], expected_hits: u64) -> Result<ExternalToolReport> {
    // nosemgrep: rust.lang.security.temp-dir.temp-dir -- benchmark scratch data is non-sensitive.
    let root = std::env::temp_dir().join(format!(
        "qzt-bench-compete-{}-{}",
        std::process::id(),
        unique_temp_suffix()
    ));
    std::fs::create_dir_all(&root).map_err(|_| QztError::ContainerCorrupt)?;
    let corpus_path = root.join("corpus.txt");
    std::fs::write(&corpus_path, corpus).map_err(|_| QztError::ContainerCorrupt)?;

    let result = (|| {
        let ripgrep_hit_count = run_ripgrep_count(&corpus_path)?;
        if let Some(count) = ripgrep_hit_count {
            if count != expected_hits {
                return Err(QztError::ContainerCorrupt);
            }
        }

        let sqlite_fts5_hit_count = run_sqlite_fts5_count(&corpus_path)?;
        if let Some(count) = sqlite_fts5_hit_count {
            if count != expected_hits {
                return Err(QztError::ContainerCorrupt);
            }
        }

        Ok(ExternalToolReport {
            ripgrep_hit_count,
            sqlite_fts5_hit_count,
        })
    })();

    let _ = std::fs::remove_dir_all(&root);
    result
}

#[cfg(feature = "bench-compete")]
fn run_ripgrep_count(path: &std::path::Path) -> Result<Option<u64>> {
    let output = match std::process::Command::new("rg")
        .arg("--fixed-strings")
        .arg("--only-matching")
        .arg("qzt")
        .arg(path)
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(QztError::ContainerCorrupt),
    };

    if !output.status.success() {
        return Ok(None);
    }

    count_output_lines(&output.stdout).map(Some)
}

#[cfg(feature = "bench-compete")]
fn run_sqlite_fts5_count(path: &std::path::Path) -> Result<Option<u64>> {
    use std::io::Write as _;

    let db_path = path.with_extension("sqlite");
    let mut child = match std::process::Command::new("sqlite3")
        .arg("-batch")
        .arg(&db_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(QztError::ContainerCorrupt),
    };

    {
        let Some(stdin) = child.stdin.as_mut() else {
            return Err(QztError::ContainerCorrupt);
        };
        writeln!(stdin, "CREATE VIRTUAL TABLE docs USING fts5(body);")
            .map_err(|_| QztError::ContainerCorrupt)?;
        writeln!(stdin, ".mode tabs").map_err(|_| QztError::ContainerCorrupt)?;
        writeln!(stdin, ".import {} docs", path.display())
            .map_err(|_| QztError::ContainerCorrupt)?;
        writeln!(stdin, "SELECT count(*) FROM docs WHERE docs MATCH 'qzt';")
            .map_err(|_| QztError::ContainerCorrupt)?;
    }

    let output = child
        .wait_with_output()
        .map_err(|_| QztError::ContainerCorrupt)?;
    if !output.status.success() {
        return Ok(None);
    }
    let text = std::str::from_utf8(&output.stdout).map_err(|_| QztError::ContainerCorrupt)?;
    let Some(line) = text.lines().rev().find(|line| !line.trim().is_empty()) else {
        return Ok(None);
    };
    line.trim()
        .parse::<u64>()
        .map(Some)
        .map_err(|_| QztError::ContainerCorrupt)
}

#[cfg(feature = "bench-compete")]
fn count_output_lines(output: &[u8]) -> Result<u64> {
    let count = output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .count();
    usize_to_u64(count)
}

#[cfg(feature = "bench-compete")]
fn unique_temp_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}
