use std::fmt;
use std::time::{Duration, Instant};

use crate::chunker::ChunkerOptions;
use crate::corpus::{generate_validation_corpus, CorpusKind, ValidationCorpusOptions};
use crate::error::{QztError, Result};
use crate::reader::{QztFileReader, QztReader};
use crate::search::{RawTokenIndex, SearchOptions, TokenIndexBuildOptions};
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

/// Competitive benchmark smoke configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompetitiveBenchmarkOptions {
    pub corpus_kind: CorpusKind,
    pub corpus_bytes: usize,
    pub chunk_size: usize,
    pub range_offset: u64,
    pub range_size: u64,
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
    pub corpus_id: &'static str,
    pub corpus_bytes: u64,
    pub qzt_bytes: u64,
    pub raw_zstd_bytes: u64,
    pub qzt_range_bytes: u64,
    pub raw_zstd_decoded_bytes: u64,
    pub qzt_range_micros: u128,
    pub raw_zstd_range_micros: u128,
    pub token_hit_count: u64,
    pub reference_hit_count: u64,
    pub external_search_tools_enabled: bool,
    pub ripgrep_hit_count: Option<u64>,
    pub sqlite_fts5_hit_count: Option<u64>,
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
    let corpus_bytes = u64::try_from(corpus.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
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
    let expected = &corpus[offset as usize..(offset + length) as usize];

    let qzt_reader = QztFileReader::open_read_at(&qzt[..], qzt.len() as u64)?;
    let started = Instant::now();
    let qzt_range = qzt_reader.read_range(offset, length)?;
    let qzt_range_elapsed = started.elapsed();
    if qzt_range != expected {
        return Err(QztError::ContainerCorrupt);
    }

    let started = Instant::now();
    let raw_decoded =
        zstd::stream::decode_all(raw_zstd.as_slice()).map_err(|_| QztError::ZstdDecodeError)?;
    let raw_elapsed = started.elapsed();
    if raw_decoded[offset as usize..(offset + length) as usize] != *expected {
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
        qzt_bytes: u64::try_from(qzt.len()).map_err(|_| QztError::ResourceLimitExceeded)?,
        raw_zstd_bytes: u64::try_from(raw_zstd.len())
            .map_err(|_| QztError::ResourceLimitExceeded)?,
        qzt_range_bytes: u64::try_from(qzt_range.len())
            .map_err(|_| QztError::ResourceLimitExceeded)?,
        raw_zstd_decoded_bytes: u64::try_from(raw_decoded.len())
            .map_err(|_| QztError::ResourceLimitExceeded)?,
        qzt_range_micros: qzt_range_elapsed.as_micros(),
        raw_zstd_range_micros: raw_elapsed.as_micros(),
        token_hit_count: token_report.metrics.verified_matches,
        reference_hit_count,
        external_search_tools_enabled: cfg!(feature = "bench-compete"),
        ripgrep_hit_count: external_report.ripgrep_hit_count,
        sqlite_fts5_hit_count: external_report.sqlite_fts5_hit_count,
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

fn count_substring(haystack: &[u8], needle: &[u8]) -> Result<u64> {
    if needle.is_empty() {
        return Ok(0);
    }
    let count = haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count();
    u64::try_from(count).map_err(|_| QztError::ResourceLimitExceeded)
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
    u64::try_from(count).map_err(|_| QztError::ResourceLimitExceeded)
}

#[cfg(feature = "bench-compete")]
fn unique_temp_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}
