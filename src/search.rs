use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::chunk_table::ChunkEntry;
use crate::error::{QztError, Result};
use crate::io::ReadAt;
use crate::primitives::{checked_logical_end, u64_to_usize, usize_to_u64};
use crate::reader::{ChunkDecodeCache, QztFileReader, QztReader};

/// Search index source text model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SearchIndexSource {
    RawUtf8,
    NormalizedUtf8,
}

/// Posting target granularity implemented by the Phase11 token MVP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostingGranularity {
    Line,
}

/// Build options for the transient raw token index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenIndexBuildOptions {
    pub source: SearchIndexSource,
    pub posting_granularity: PostingGranularity,
}

impl Default for TokenIndexBuildOptions {
    fn default() -> Self {
        Self {
            source: SearchIndexSource::RawUtf8,
            posting_granularity: PostingGranularity::Line,
        }
    }
}

/// N-gram unit used by the raw n-gram index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NgramUnit {
    UnicodeScalar,
}

/// Build options for the transient raw n-gram index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NgramIndexBuildOptions {
    pub source: SearchIndexSource,
    pub posting_granularity: PostingGranularity,
    pub n: usize,
    pub complete: bool,
    pub high_df_per_million: u32,
}

impl Default for NgramIndexBuildOptions {
    fn default() -> Self {
        Self {
            source: SearchIndexSource::RawUtf8,
            posting_granularity: PostingGranularity::Line,
            n: 3,
            complete: true,
            high_df_per_million: 200_000,
        }
    }
}

/// Declared n-gram interpretation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NgramDeclaration {
    pub n: usize,
    pub unit: NgramUnit,
    pub normalization: &'static str,
    pub case_fold: bool,
    pub boundary_mode: &'static str,
    pub boundary_window_bytes: u64,
}

/// Planner configuration for candidate limits and high-DF behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannerConfig {
    pub max_candidate_granules_default: u64,
    pub max_decoded_bytes_default: u64,
    pub high_df_per_million: u32,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            max_candidate_granules_default: 10_000,
            max_decoded_bytes_default: 256 * 1024 * 1024,
            high_df_per_million: 200_000,
        }
    }
}

/// Skip point metadata for long posting lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipPoint {
    pub entry_index: u64,
    pub granule_id: u64,
    pub posting_byte_offset: u64,
}

/// Inspectable query planner decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerDecision {
    /// All index keys required to answer the query (one per query token/ngram).
    pub required_keys: Vec<Vec<u8>>,
    /// Subset of required keys that were found in the index and used for posting intersection.
    pub selected_keys: Vec<Vec<u8>>,
    /// Required keys absent from the index (query returns no hits when non-empty).
    pub missing_keys: Vec<Vec<u8>>,
    /// Selected keys whose document frequency exceeds the high-DF threshold.
    pub high_df_keys: Vec<Vec<u8>>,
    /// Whether skip-point data was used to accelerate posting list traversal.
    pub used_skip_data: bool,
}

impl PlannerDecision {
    pub(crate) fn new(required_keys: Vec<Vec<u8>>) -> Self {
        Self {
            required_keys,
            selected_keys: Vec::new(),
            missing_keys: Vec::new(),
            high_df_keys: Vec::new(),
            used_skip_data: false,
        }
    }
}

/// Runtime search limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchOptions {
    pub max_candidate_granules: u64,
    pub max_decoded_bytes: u64,
    pub max_search_results: u64,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_candidate_granules: 10_000,
            max_decoded_bytes: 256 * 1024 * 1024,
            max_search_results: u64::MAX,
        }
    }
}

/// One posting target over original bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchGranule {
    pub granule_id: u64,
    pub logical_offset: u64,
    pub byte_length: u64,
    pub chunk_start: u64,
    pub chunk_end: u64,
    pub first_line: Option<u64>,
    pub line_count: Option<u64>,
}

/// Logical term dictionary entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermDictionaryEntry {
    pub key: Vec<u8>,
    pub key_hash: [u8; 16],
    pub document_frequency: u64,
    pub granule_frequency: u64,
    pub posting_offset: u64,
    pub posting_size: u64,
    pub skip_offset: u64,
    pub skip_size: u64,
    pub flags: u64,
}

/// One verified search hit.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub logical_offset: u64,
    pub byte_length: u64,
    pub chunk_start: u64,
    pub chunk_end: u64,
    pub score: Option<f64>,
    pub source: &'static str,
}

/// Search metrics required for benchmark and debug output.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchMetrics {
    pub query: String,
    pub index_kind: &'static str,
    pub posting_granularity: &'static str,
    pub index_size_bytes: u64,
    pub source_size_bytes: u64,
    pub index_size_ratio: f64,
    pub term_lookups: u64,
    pub posting_bytes_read: u64,
    pub candidate_granules: u64,
    pub candidate_chunks: u64,
    pub decoded_bytes: u64,
    /// Total uncompressed bytes physically decompressed during hit
    /// verification (chunk-level work, as opposed to the logical granule
    /// bytes counted by `decoded_bytes`).
    pub physical_decoded_bytes: u64,
    pub verified_matches: u64,
    pub query_time_ms: f64,
}

/// Search result plus metrics. `capped` means limits stopped verification.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchReport {
    pub hits: Vec<SearchHit>,
    pub metrics: SearchMetrics,
    pub capped: bool,
    pub planner: PlannerDecision,
    pub incomplete_reason: Option<&'static str>,
}

/// Transient raw token index over line granules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTokenIndex {
    pub container_id: [u8; 16],
    pub source_size_bytes: u64,
    pub source: SearchIndexSource,
    pub posting_granularity: PostingGranularity,
    pub complete: bool,
    pub granules: Vec<SearchGranule>,
    pub terms: Vec<TermDictionaryEntry>,
    pub postings: Vec<Vec<u64>>,
    encoded_postings: Vec<Vec<u8>>,
}

impl RawTokenIndex {
    pub fn build_from_container(bytes: &[u8], options: TokenIndexBuildOptions) -> Result<Self> {
        let len = usize_to_u64(bytes.len())?;
        let reader = QztFileReader::open_read_at(bytes, len)?;
        Self::build_from_file(&reader, options)
    }

    /// Builds the index by decoding the container one chunk at a time, so the
    /// full original text is never held in memory.
    pub fn build_from_file<R: ReadAt>(
        reader: &QztFileReader<R>,
        options: TokenIndexBuildOptions,
    ) -> Result<Self> {
        if options.source == SearchIndexSource::NormalizedUtf8 {
            return Err(QztError::UnsupportedIndexMode(
                "normalized_utf8 token index",
            ));
        }

        let details = reader.skeleton_details();
        let (granules, terms, postings) = match options.posting_granularity {
            PostingGranularity::Line => build_line_index_streaming(
                &details.chunk_entries,
                details.summary.original_size,
                |entry| reader.decode_entry(entry),
                |line| {
                    Ok(tokenize_ascii_lower(line)
                        .into_iter()
                        .map(|token| token.key)
                        .collect())
                },
            )?,
        };
        Self::from_parts(
            details.summary.container_id,
            details.summary.original_size,
            granules,
            terms,
            postings,
        )
    }

    pub fn from_parts(
        container_id: [u8; 16],
        source_size_bytes: u64,
        granules: Vec<SearchGranule>,
        mut terms: Vec<TermDictionaryEntry>,
        postings: Vec<Vec<u64>>,
    ) -> Result<Self> {
        validate_granules(source_size_bytes, &granules)?;
        validate_term_dictionary_shape(&terms, &postings, granules.len())?;

        let mut encoded_postings = Vec::with_capacity(postings.len());
        let mut posting_offset = 0_u64;
        for (term, posting_list) in terms.iter_mut().zip(&postings) {
            let encoded = encode_delta_varint_u64(posting_list)?;
            term.document_frequency = 0;
            term.granule_frequency = usize_to_u64(posting_list.len())?;
            term.posting_offset = posting_offset;
            term.posting_size = usize_to_u64(encoded.len())?;
            term.skip_offset = 0;
            term.skip_size = 0;
            posting_offset = posting_offset
                .checked_add(term.posting_size)
                .ok_or(QztError::ResourceLimitExceeded)?;
            encoded_postings.push(encoded);
        }

        Ok(Self {
            container_id,
            source_size_bytes,
            source: SearchIndexSource::RawUtf8,
            posting_granularity: PostingGranularity::Line,
            complete: true,
            granules,
            terms,
            postings,
            encoded_postings,
        })
    }

    #[cfg(feature = "internal-testing")]
    pub fn posting_list_for_key(&self, key: &[u8]) -> Option<&[u64]> {
        self.term_index_for_key(key)
            .and_then(|index| self.postings.get(index).map(Vec::as_slice))
    }

    pub fn search(
        &self,
        reader: &QztReader,
        query: &str,
        options: SearchOptions,
    ) -> Result<SearchReport> {
        self.search_impl(query, options, &mut |offset, length, cache| {
            reader.read_range_cached(offset, length, cache)
        })
    }

    /// Search over a file-backed container, decoding only candidate chunks.
    pub fn search_file<R: ReadAt>(
        &self,
        reader: &QztFileReader<R>,
        query: &str,
        options: SearchOptions,
    ) -> Result<SearchReport> {
        self.search_impl(query, options, &mut |offset, length, cache| {
            reader.read_range_cached(offset, length, cache)
        })
    }

    fn search_impl(
        &self,
        query: &str,
        options: SearchOptions,
        read_range_cached: RangeReadFn<'_>,
    ) -> Result<SearchReport> {
        let started = Instant::now();
        let query_keys = unique_query_keys(query.as_bytes());
        let mut planner = PlannerDecision::new(query_keys.clone());
        let mut metrics = self.empty_metrics(query);
        metrics.term_lookups = usize_to_u64(query_keys.len())?;

        if query_keys.is_empty() {
            metrics.query_time_ms = elapsed_ms(started);
            return Ok(SearchReport {
                hits: Vec::new(),
                metrics,
                capped: false,
                planner,
                incomplete_reason: Some("query_has_no_indexable_tokens"),
            });
        }

        let mut posting_indexes = Vec::with_capacity(query_keys.len());
        for key in &query_keys {
            let Some(term_index) = self.term_index_for_key(key) else {
                planner.missing_keys.push(key.clone());
                metrics.query_time_ms = elapsed_ms(started);
                return Ok(SearchReport {
                    hits: Vec::new(),
                    metrics,
                    capped: false,
                    planner,
                    incomplete_reason: None,
                });
            };
            metrics.posting_bytes_read = metrics
                .posting_bytes_read
                .checked_add(self.terms[term_index].posting_size)
                .ok_or(QztError::ResourceLimitExceeded)?;
            posting_indexes.push(term_index);
        }

        posting_indexes.sort_by_key(|index| self.postings[*index].len());
        planner.selected_keys = posting_indexes
            .iter()
            .map(|index| self.terms[*index].key.clone())
            .collect();
        let posting_refs = posting_indexes
            .iter()
            .map(|index| self.postings[*index].as_slice())
            .collect::<Vec<_>>();
        let candidates = intersect_postings(&posting_refs);
        metrics.candidate_granules = usize_to_u64(candidates.len())?;
        metrics.candidate_chunks = count_candidate_chunks(&self.granules, &candidates)?;

        if metrics.candidate_granules > options.max_candidate_granules {
            metrics.query_time_ms = elapsed_ms(started);
            return Ok(SearchReport {
                hits: Vec::new(),
                metrics,
                capped: true,
                planner,
                incomplete_reason: None,
            });
        }
        if options.max_search_results == 0 {
            metrics.query_time_ms = elapsed_ms(started);
            return Ok(SearchReport {
                hits: Vec::new(),
                metrics,
                capped: true,
                planner,
                incomplete_reason: None,
            });
        }

        let verification = verify_candidates(
            &candidates,
            &mut |granule_id| {
                let granule_index = u64_to_usize(granule_id)?;
                self.granules
                    .get(granule_index)
                    .cloned()
                    .ok_or(QztError::ContainerCorrupt)
            },
            read_range_cached,
            &mut |decoded| verified_spans(decoded, &query_keys),
            options,
        )?;

        metrics.decoded_bytes = verification.decoded_bytes;
        metrics.physical_decoded_bytes = verification.physical_decoded_bytes;
        metrics.verified_matches = usize_to_u64(verification.hits.len())?;
        metrics.query_time_ms = elapsed_ms(started);
        Ok(SearchReport {
            hits: verification.hits,
            metrics,
            capped: verification.capped,
            planner,
            incomplete_reason: None,
        })
    }

    fn term_index_for_key(&self, key: &[u8]) -> Option<usize> {
        let key_hash = key_hash(key);
        self.terms
            .iter()
            .position(|term| term.key_hash == key_hash && term.key == key)
    }

    fn empty_metrics(&self, query: &str) -> SearchMetrics {
        let index_size_bytes = self.index_size_bytes();
        let index_size_ratio = if self.source_size_bytes == 0 {
            0.0
        } else {
            index_size_bytes as f64 / self.source_size_bytes as f64
        };

        SearchMetrics {
            query: query.to_owned(),
            index_kind: "token",
            posting_granularity: "line",
            index_size_bytes,
            source_size_bytes: self.source_size_bytes,
            index_size_ratio,
            term_lookups: 0,
            posting_bytes_read: 0,
            candidate_granules: 0,
            candidate_chunks: 0,
            decoded_bytes: 0,
            physical_decoded_bytes: 0,
            verified_matches: 0,
            query_time_ms: 0.0,
        }
    }

    fn index_size_bytes(&self) -> u64 {
        let granule_bytes = self.granules.len().saturating_mul(56);
        let term_bytes = self
            .terms
            .iter()
            .map(|term| term.key.len().saturating_add(80))
            .sum::<usize>();
        let posting_bytes = self.encoded_postings.iter().map(Vec::len).sum::<usize>();
        u64::try_from(
            granule_bytes
                .saturating_add(term_bytes)
                .saturating_add(posting_bytes),
        )
        .unwrap_or(u64::MAX)
    }
}

/// Transient raw Unicode-scalar n-gram index over line granules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawNgramIndex {
    pub container_id: [u8; 16],
    pub source_size_bytes: u64,
    pub source: SearchIndexSource,
    pub posting_granularity: PostingGranularity,
    pub complete: bool,
    pub declaration: NgramDeclaration,
    pub planner_config: PlannerConfig,
    pub granules: Vec<SearchGranule>,
    pub terms: Vec<TermDictionaryEntry>,
    pub postings: Vec<Vec<u64>>,
    pub skip_data: Vec<Vec<SkipPoint>>,
    encoded_postings: Vec<Vec<u8>>,
}

impl RawNgramIndex {
    pub fn build_from_container(bytes: &[u8], options: NgramIndexBuildOptions) -> Result<Self> {
        let len = usize_to_u64(bytes.len())?;
        let reader = QztFileReader::open_read_at(bytes, len)?;
        Self::build_from_file(&reader, options)
    }

    /// Builds the index by decoding the container one chunk at a time, so the
    /// full original text is never held in memory.
    pub fn build_from_file<R: ReadAt>(
        reader: &QztFileReader<R>,
        options: NgramIndexBuildOptions,
    ) -> Result<Self> {
        if options.source == SearchIndexSource::NormalizedUtf8 {
            return Err(QztError::UnsupportedIndexMode(
                "normalized_utf8 ngram index",
            ));
        }
        if options.n == 0 {
            return Err(QztError::ResourceLimitExceeded);
        }

        let details = reader.skeleton_details();
        let (granules, terms, postings) = match options.posting_granularity {
            PostingGranularity::Line => build_line_index_streaming(
                &details.chunk_entries,
                details.summary.original_size,
                |entry| reader.decode_entry(entry),
                |line| {
                    let text = std::str::from_utf8(line).map_err(|_| QztError::InvalidUtf8)?;
                    Ok(ngram_keys(text, options.n))
                },
            )?,
        };
        Self::from_parts(
            details.summary.container_id,
            details.summary.original_size,
            granules,
            terms,
            postings,
            options,
        )
    }

    pub fn from_parts(
        container_id: [u8; 16],
        source_size_bytes: u64,
        granules: Vec<SearchGranule>,
        mut terms: Vec<TermDictionaryEntry>,
        postings: Vec<Vec<u64>>,
        options: NgramIndexBuildOptions,
    ) -> Result<Self> {
        validate_granules(source_size_bytes, &granules)?;
        validate_term_dictionary_shape(&terms, &postings, granules.len())?;

        let mut encoded_postings = Vec::with_capacity(postings.len());
        let mut skip_data = Vec::with_capacity(postings.len());
        let mut posting_offset = 0_u64;
        let mut skip_offset = 0_u64;
        for (term, posting_list) in terms.iter_mut().zip(&postings) {
            let encoded = encode_delta_varint_u64(posting_list)?;
            let skips = build_skip_points(posting_list)?;
            term.document_frequency = 0;
            term.granule_frequency = usize_to_u64(posting_list.len())?;
            term.posting_offset = posting_offset;
            term.posting_size = usize_to_u64(encoded.len())?;
            term.skip_offset = skip_offset;
            term.skip_size = usize_to_u64(skips.len().saturating_mul(24))?;
            posting_offset = posting_offset
                .checked_add(term.posting_size)
                .ok_or(QztError::ResourceLimitExceeded)?;
            skip_offset = skip_offset
                .checked_add(term.skip_size)
                .ok_or(QztError::ResourceLimitExceeded)?;
            encoded_postings.push(encoded);
            skip_data.push(skips);
        }

        Ok(Self {
            container_id,
            source_size_bytes,
            source: SearchIndexSource::RawUtf8,
            posting_granularity: PostingGranularity::Line,
            complete: options.complete,
            declaration: NgramDeclaration {
                n: options.n,
                unit: NgramUnit::UnicodeScalar,
                normalization: "none",
                case_fold: false,
                boundary_mode: "adjacent_decode",
                boundary_window_bytes: 4096,
            },
            planner_config: PlannerConfig {
                high_df_per_million: options.high_df_per_million,
                ..PlannerConfig::default()
            },
            granules,
            terms,
            postings,
            skip_data,
            encoded_postings,
        })
    }

    #[cfg(feature = "internal-testing")]
    pub fn term_for_key(&self, key: &[u8]) -> Option<&TermDictionaryEntry> {
        self.term_index_for_key(key)
            .and_then(|index| self.terms.get(index))
    }

    pub fn search(
        &self,
        reader: &QztReader,
        query: &str,
        options: SearchOptions,
    ) -> Result<SearchReport> {
        self.search_impl(query, options, &mut |offset, length, cache| {
            reader.read_range_cached(offset, length, cache)
        })
    }

    /// Search over a file-backed container, decoding only candidate chunks.
    pub fn search_file<R: ReadAt>(
        &self,
        reader: &QztFileReader<R>,
        query: &str,
        options: SearchOptions,
    ) -> Result<SearchReport> {
        self.search_impl(query, options, &mut |offset, length, cache| {
            reader.read_range_cached(offset, length, cache)
        })
    }

    fn search_impl(
        &self,
        query: &str,
        options: SearchOptions,
        read_range_cached: RangeReadFn<'_>,
    ) -> Result<SearchReport> {
        let started = Instant::now();
        let query_keys = ngram_keys_for_query(query, self.declaration.n)?;
        let mut planner = PlannerDecision::new(query_keys.clone());
        let mut metrics = self.empty_metrics(query);
        metrics.term_lookups = usize_to_u64(query_keys.len())?;

        if query_keys.is_empty() {
            metrics.query_time_ms = elapsed_ms(started);
            return Ok(SearchReport {
                hits: Vec::new(),
                metrics,
                capped: false,
                planner,
                incomplete_reason: Some("query_shorter_than_ngram_n"),
            });
        }

        let mut term_indexes = Vec::with_capacity(query_keys.len());
        for key in &query_keys {
            let Some(term_index) = self.term_index_for_key(key) else {
                planner.missing_keys.push(key.clone());
                metrics.query_time_ms = elapsed_ms(started);
                return Ok(SearchReport {
                    hits: Vec::new(),
                    metrics,
                    capped: false,
                    planner,
                    incomplete_reason: (!self.complete)
                        .then_some("missing_required_key_in_incomplete_index"),
                });
            };
            term_indexes.push(term_index);
        }

        term_indexes.sort_by(|left, right| {
            let left_high = self.is_high_df(*left);
            let right_high = self.is_high_df(*right);
            (left_high, self.terms[*left].granule_frequency)
                .cmp(&(right_high, self.terms[*right].granule_frequency))
        });
        for term_index in &term_indexes {
            if self.is_high_df(*term_index) {
                planner
                    .high_df_keys
                    .push(self.terms[*term_index].key.clone());
            }
            if !self.skip_data[*term_index].is_empty() {
                planner.used_skip_data = true;
            }
        }
        planner.selected_keys = term_indexes
            .iter()
            .map(|index| self.terms[*index].key.clone())
            .collect();

        for term_index in &term_indexes {
            metrics.posting_bytes_read = metrics
                .posting_bytes_read
                .checked_add(self.reported_posting_bytes_read(*term_index)?)
                .ok_or(QztError::ResourceLimitExceeded)?;
        }

        let posting_refs = term_indexes
            .iter()
            .map(|index| self.postings[*index].as_slice())
            .collect::<Vec<_>>();
        let candidates = intersect_postings(&posting_refs);
        metrics.candidate_granules = usize_to_u64(candidates.len())?;
        metrics.candidate_chunks = count_candidate_chunks(&self.granules, &candidates)?;

        if metrics.candidate_granules > options.max_candidate_granules {
            metrics.query_time_ms = elapsed_ms(started);
            return Ok(SearchReport {
                hits: Vec::new(),
                metrics,
                capped: true,
                planner,
                incomplete_reason: None,
            });
        }
        if options.max_search_results == 0 {
            metrics.query_time_ms = elapsed_ms(started);
            return Ok(SearchReport {
                hits: Vec::new(),
                metrics,
                capped: true,
                planner,
                incomplete_reason: None,
            });
        }

        let verification = verify_candidates(
            &candidates,
            &mut |granule_id| {
                let granule_index = u64_to_usize(granule_id)?;
                self.granules
                    .get(granule_index)
                    .cloned()
                    .ok_or(QztError::ContainerCorrupt)
            },
            read_range_cached,
            &mut |decoded| substring_spans(decoded, query.as_bytes()),
            options,
        )?;

        metrics.decoded_bytes = verification.decoded_bytes;
        metrics.physical_decoded_bytes = verification.physical_decoded_bytes;
        metrics.verified_matches = usize_to_u64(verification.hits.len())?;
        metrics.query_time_ms = elapsed_ms(started);
        Ok(SearchReport {
            hits: verification.hits,
            metrics,
            capped: verification.capped,
            planner,
            incomplete_reason: None,
        })
    }

    fn term_index_for_key(&self, key: &[u8]) -> Option<usize> {
        let key_hash = key_hash(key);
        self.terms
            .iter()
            .position(|term| term.key_hash == key_hash && term.key == key)
    }

    fn is_high_df(&self, term_index: usize) -> bool {
        let granule_count = self.granules.len().max(1) as u128;
        let frequency = u128::from(self.terms[term_index].granule_frequency);
        let per_million = frequency.saturating_mul(1_000_000) / granule_count;
        per_million >= u128::from(self.planner_config.high_df_per_million)
    }

    fn reported_posting_bytes_read(&self, term_index: usize) -> Result<u64> {
        let term = &self.terms[term_index];
        if self.skip_data[term_index].is_empty() {
            return Ok(term.posting_size);
        }
        let skip_probe_bytes = usize_to_u64(self.skip_data[term_index].len().saturating_mul(24))?
            .checked_add(16)
            .ok_or(QztError::ResourceLimitExceeded)?;
        Ok(skip_probe_bytes.min(term.posting_size))
    }

    fn empty_metrics(&self, query: &str) -> SearchMetrics {
        let index_size_bytes = self.index_size_bytes();
        let index_size_ratio = if self.source_size_bytes == 0 {
            0.0
        } else {
            index_size_bytes as f64 / self.source_size_bytes as f64
        };

        SearchMetrics {
            query: query.to_owned(),
            index_kind: "ngram",
            posting_granularity: "line",
            index_size_bytes,
            source_size_bytes: self.source_size_bytes,
            index_size_ratio,
            term_lookups: 0,
            posting_bytes_read: 0,
            candidate_granules: 0,
            candidate_chunks: 0,
            decoded_bytes: 0,
            physical_decoded_bytes: 0,
            verified_matches: 0,
            query_time_ms: 0.0,
        }
    }

    fn index_size_bytes(&self) -> u64 {
        let granule_bytes = self.granules.len().saturating_mul(56);
        let term_bytes = self
            .terms
            .iter()
            .map(|term| term.key.len().saturating_add(80))
            .sum::<usize>();
        let posting_bytes = self.encoded_postings.iter().map(Vec::len).sum::<usize>();
        let skip_bytes = self.skip_data.iter().flatten().count().saturating_mul(24);
        u64::try_from(
            granule_bytes
                .saturating_add(term_bytes)
                .saturating_add(posting_bytes)
                .saturating_add(skip_bytes),
        )
        .unwrap_or(u64::MAX)
    }
}

pub fn encode_delta_varint_u64(values: &[u64]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut previous = 0_u64;
    for (index, value) in values.iter().enumerate() {
        let delta = if index == 0 {
            *value
        } else {
            if *value <= previous {
                return Err(QztError::ContainerCorrupt);
            }
            value
                .checked_sub(previous)
                .ok_or(QztError::ContainerCorrupt)?
        };
        write_varuint(delta, &mut bytes);
        previous = *value;
    }
    Ok(bytes)
}

pub fn decode_delta_varint_u64(bytes: &[u8]) -> Result<Vec<u64>> {
    let mut cursor = 0_usize;
    let mut values = Vec::new();
    let mut previous = 0_u64;
    while cursor < bytes.len() {
        let delta = read_varuint(bytes, &mut cursor)?;
        let value = if values.is_empty() {
            delta
        } else {
            previous
                .checked_add(delta)
                .ok_or(QztError::ContainerCorrupt)?
        };
        if !values.is_empty() && value <= previous {
            return Err(QztError::ContainerCorrupt);
        }
        values.push(value);
        previous = value;
    }
    Ok(values)
}

/// Granules, sorted term dictionary, and per-term posting lists.
type LineIndexParts = (Vec<SearchGranule>, Vec<TermDictionaryEntry>, Vec<Vec<u64>>);

/// Builds line granules and a sorted term dictionary in one pass over the
/// container chunks. Only one decoded chunk plus the trailing incomplete line
/// is held at a time; the posting map still grows with vocabulary.
fn build_line_index_streaming(
    entries: &[ChunkEntry],
    original_size: u64,
    mut decode: impl FnMut(&ChunkEntry) -> Result<Vec<u8>>,
    mut keys_for_line: impl FnMut(&[u8]) -> Result<Vec<Vec<u8>>>,
) -> Result<LineIndexParts> {
    let mut postings_by_key: BTreeMap<Vec<u8>, BTreeSet<u64>> = BTreeMap::new();
    let mut granules: Vec<SearchGranule> = Vec::new();
    let mut carry: Vec<u8> = Vec::new();
    let mut line_start = 0_u64;

    for entry in entries {
        let decoded = decode(entry)?;
        // Chunk boundaries are UTF-8 safe, so validating per chunk is
        // equivalent to validating the whole original text.
        std::str::from_utf8(&decoded).map_err(|_| QztError::InvalidUtf8)?;

        let mut consumed = 0_usize;
        for (index, byte) in decoded.iter().enumerate() {
            if *byte != b'\n' {
                continue;
            }
            let line_end = checked_logical_end(entry.logical_offset, usize_to_u64(index + 1)?)?;
            let line_bytes: &[u8] = if carry.is_empty() {
                &decoded[consumed..=index]
            } else {
                carry.extend_from_slice(&decoded[consumed..=index]);
                &carry
            };
            emit_line_granule(
                entries,
                line_start,
                line_end,
                line_bytes,
                &mut granules,
                &mut postings_by_key,
                &mut keys_for_line,
            )?;
            carry.clear();
            consumed = index + 1;
            line_start = line_end;
        }
        if consumed < decoded.len() {
            carry.extend_from_slice(&decoded[consumed..]);
        }
    }

    if !carry.is_empty() {
        let line_bytes = std::mem::take(&mut carry);
        emit_line_granule(
            entries,
            line_start,
            original_size,
            &line_bytes,
            &mut granules,
            &mut postings_by_key,
            &mut keys_for_line,
        )?;
    }

    let mut terms = Vec::with_capacity(postings_by_key.len());
    let mut postings = Vec::with_capacity(postings_by_key.len());
    for (key, posting_set) in postings_by_key {
        terms.push(TermDictionaryEntry {
            key: key.clone(),
            key_hash: key_hash(&key),
            document_frequency: 0,
            granule_frequency: 0,
            posting_offset: 0,
            posting_size: 0,
            skip_offset: 0,
            skip_size: 0,
            flags: 0,
        });
        postings.push(posting_set.into_iter().collect());
    }
    Ok((granules, terms, postings))
}

fn emit_line_granule(
    entries: &[ChunkEntry],
    line_start: u64,
    line_end: u64,
    line_bytes: &[u8],
    granules: &mut Vec<SearchGranule>,
    postings_by_key: &mut BTreeMap<Vec<u8>, BTreeSet<u64>>,
    keys_for_line: &mut impl FnMut(&[u8]) -> Result<Vec<Vec<u8>>>,
) -> Result<()> {
    let granule_id = usize_to_u64(granules.len())?;
    let byte_length = line_end
        .checked_sub(line_start)
        .ok_or(QztError::LogicalRangeOutOfBounds)?;
    let (chunk_start, chunk_end) = chunk_span_for_range(entries, line_start, line_end)?;
    granules.push(SearchGranule {
        granule_id,
        logical_offset: line_start,
        byte_length,
        chunk_start,
        chunk_end,
        first_line: Some(granule_id),
        line_count: Some(1),
    });
    for key in keys_for_line(line_bytes)? {
        postings_by_key.entry(key).or_default().insert(granule_id);
    }
    Ok(())
}

/// Chunk-id span `[chunk_start, chunk_end)` covering a non-empty logical
/// range, found with two binary searches over the contiguous chunk table.
fn chunk_span_for_range(entries: &[ChunkEntry], start: u64, end: u64) -> Result<(u64, u64)> {
    let first_index = chunk_index_for_offset(entries, start)?;
    let last_index = chunk_index_for_offset(entries, end.saturating_sub(1))?;
    let first = entries
        .get(first_index)
        .ok_or(QztError::ChunkTableInvalid)?
        .chunk_id;
    let last = entries
        .get(last_index)
        .ok_or(QztError::ChunkTableInvalid)?
        .chunk_id;
    let last_exclusive = last.checked_add(1).ok_or(QztError::ChunkTableInvalid)?;
    Ok((first, last_exclusive))
}

fn chunk_index_for_offset(entries: &[ChunkEntry], offset: u64) -> Result<usize> {
    let mut low = 0_usize;
    let mut high = entries.len();
    while low < high {
        let mid = low + (high - low) / 2;
        let chunk_end =
            checked_logical_end(entries[mid].logical_offset, entries[mid].uncompressed_size)?;
        if chunk_end <= offset {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    Ok(low)
}

fn build_skip_points(posting_list: &[u64]) -> Result<Vec<SkipPoint>> {
    if posting_list.len() < 1024 {
        return Ok(Vec::new());
    }

    let mut points = Vec::new();
    let mut encoded = Vec::new();
    let mut previous = 0_u64;
    for (index, granule_id) in posting_list.iter().enumerate() {
        let byte_offset = usize_to_u64(encoded.len())?;
        if index > 0 && index % 128 == 0 {
            points.push(SkipPoint {
                entry_index: usize_to_u64(index)?,
                granule_id: *granule_id,
                posting_byte_offset: byte_offset,
            });
        }
        let delta = if index == 0 {
            *granule_id
        } else {
            if *granule_id <= previous {
                return Err(QztError::ContainerCorrupt);
            }
            granule_id
                .checked_sub(previous)
                .ok_or(QztError::ContainerCorrupt)?
        };
        write_varuint(delta, &mut encoded);
        previous = *granule_id;
    }
    Ok(points)
}

fn validate_granules(source_size_bytes: u64, granules: &[SearchGranule]) -> Result<()> {
    let mut previous_offset = None;
    for (index, granule) in granules.iter().enumerate() {
        if granule.granule_id != index as u64 {
            return Err(QztError::ContainerCorrupt);
        }
        let end = checked_logical_end(granule.logical_offset, granule.byte_length)?;
        if end > source_size_bytes {
            return Err(QztError::LogicalRangeOutOfBounds);
        }
        if let Some(previous_offset) = previous_offset {
            if granule.logical_offset < previous_offset {
                return Err(QztError::ContainerCorrupt);
            }
        }
        if granule.chunk_end < granule.chunk_start {
            return Err(QztError::ChunkTableInvalid);
        }
        previous_offset = Some(granule.logical_offset);
    }
    Ok(())
}

fn validate_term_dictionary_shape(
    terms: &[TermDictionaryEntry],
    postings: &[Vec<u64>],
    granule_count: usize,
) -> Result<()> {
    if terms.len() != postings.len() {
        return Err(QztError::ContainerCorrupt);
    }
    if !terms.windows(2).all(|pair| pair[0].key < pair[1].key) {
        return Err(QztError::ContainerCorrupt);
    }
    for (term, posting_list) in terms.iter().zip(postings) {
        if term.flags != 0 {
            return Err(QztError::InvalidFlags);
        }
        for pair in posting_list.windows(2) {
            if pair[0] >= pair[1] {
                return Err(QztError::ContainerCorrupt);
            }
        }
        for granule_id in posting_list {
            let granule_index = u64_to_usize(*granule_id)?;
            if granule_index >= granule_count {
                return Err(QztError::ContainerCorrupt);
            }
        }
    }
    Ok(())
}

/// Shared signature for cached range reads used during hit verification.
pub(crate) type RangeReadFn<'a> =
    &'a mut dyn FnMut(u64, u64, &mut ChunkDecodeCache) -> Result<Vec<u8>>;

/// Outcome of verifying candidate granules against original bytes.
pub(crate) struct CandidateVerification {
    pub(crate) hits: Vec<SearchHit>,
    pub(crate) capped: bool,
    pub(crate) decoded_bytes: u64,
    pub(crate) physical_decoded_bytes: u64,
}

/// Decodes each candidate granule (chunk decode cache shared across the loop)
/// and confirms matches against original bytes. Shared by the in-memory
/// indexes and the file-backed sidecar search.
pub(crate) fn verify_candidates(
    candidates: &[u64],
    granule_at: &mut dyn FnMut(u64) -> Result<SearchGranule>,
    read_range_cached: RangeReadFn<'_>,
    spans_for: &mut dyn FnMut(&[u8]) -> Vec<TokenSpan>,
    options: SearchOptions,
) -> Result<CandidateVerification> {
    let mut hits = Vec::new();
    let mut capped = false;
    let mut decoded_bytes = 0_u64;
    let mut cache = ChunkDecodeCache::new();
    for granule_id in candidates {
        let granule = granule_at(*granule_id)?;
        let next_decoded = decoded_bytes
            .checked_add(granule.byte_length)
            .ok_or(QztError::ResourceLimitExceeded)?;
        if next_decoded > options.max_decoded_bytes {
            capped = true;
            break;
        }

        let decoded = read_range_cached(granule.logical_offset, granule.byte_length, &mut cache)?;
        decoded_bytes = next_decoded;
        for span in spans_for(&decoded) {
            let span_offset = usize_to_u64(span.start)?;
            let span_len = usize_to_u64(span.end - span.start)?;
            hits.push(SearchHit {
                logical_offset: granule
                    .logical_offset
                    .checked_add(span_offset)
                    .ok_or(QztError::LogicalRangeOutOfBounds)?,
                byte_length: span_len,
                chunk_start: granule.chunk_start,
                chunk_end: granule.chunk_end,
                score: None,
                source: "verified_original_bytes",
            });
            if usize_to_u64(hits.len())? >= options.max_search_results {
                capped = true;
                break;
            }
        }
        if capped {
            break;
        }
    }
    Ok(CandidateVerification {
        hits,
        capped,
        decoded_bytes,
        physical_decoded_bytes: cache.physical_decoded_bytes(),
    })
}

fn count_candidate_chunks(granules: &[SearchGranule], candidates: &[u64]) -> Result<u64> {
    let mut chunks = BTreeSet::new();
    for granule_id in candidates {
        let granule_index = u64_to_usize(*granule_id)?;
        let granule = granules
            .get(granule_index)
            .ok_or(QztError::ContainerCorrupt)?;
        for chunk_id in granule.chunk_start..granule.chunk_end {
            chunks.insert(chunk_id);
        }
    }
    usize_to_u64(chunks.len())
}

pub(crate) fn intersect_postings(posting_lists: &[&[u64]]) -> Vec<u64> {
    let Some(first) = posting_lists.first() else {
        return Vec::new();
    };
    let mut current = first.to_vec();
    for posting_list in &posting_lists[1..] {
        current = intersect_two_sorted(&current, posting_list);
        if current.is_empty() {
            break;
        }
    }
    current
}

fn intersect_two_sorted(left: &[u64], right: &[u64]) -> Vec<u64> {
    let mut output = Vec::new();
    let mut left_index = 0_usize;
    let mut right_index = 0_usize;
    while left_index < left.len() && right_index < right.len() {
        match left[left_index].cmp(&right[right_index]) {
            std::cmp::Ordering::Less => left_index += 1,
            std::cmp::Ordering::Greater => right_index += 1,
            std::cmp::Ordering::Equal => {
                output.push(left[left_index]);
                left_index += 1;
                right_index += 1;
            }
        }
    }
    output
}

pub(crate) fn verified_spans(bytes: &[u8], query_keys: &[Vec<u8>]) -> Vec<TokenSpan> {
    let tokens = tokenize_ascii_lower(bytes);
    if query_keys
        .iter()
        .all(|key| tokens.iter().any(|token| token.key == *key))
    {
        tokens
            .into_iter()
            .filter(|token| query_keys.contains(&token.key))
            .collect()
    } else {
        Vec::new()
    }
}

pub(crate) fn substring_spans(bytes: &[u8], query: &[u8]) -> Vec<TokenSpan> {
    if query.is_empty() || query.len() > bytes.len() {
        return Vec::new();
    }

    let mut spans = Vec::new();
    for start in 0..=bytes.len() - query.len() {
        let end = start + query.len();
        if &bytes[start..end] == query {
            spans.push(TokenSpan {
                key: query.to_vec(),
                start,
                end,
            });
        }
    }
    spans
}

pub(crate) fn unique_query_keys(query: &[u8]) -> Vec<Vec<u8>> {
    let mut keys = tokenize_ascii_lower(query)
        .into_iter()
        .map(|token| token.key)
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    keys
}

pub(crate) fn ngram_keys_for_query(query: &str, n: usize) -> Result<Vec<Vec<u8>>> {
    if n == 0 {
        return Err(QztError::ResourceLimitExceeded);
    }
    let mut keys = ngram_keys(query, n);
    keys.sort();
    keys.dedup();
    Ok(keys)
}

fn ngram_keys(text: &str, n: usize) -> Vec<Vec<u8>> {
    if n == 0 {
        return Vec::new();
    }
    let char_starts = text
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(text.len()))
        .collect::<Vec<_>>();
    if char_starts.len().saturating_sub(1) < n {
        return Vec::new();
    }

    let mut keys = Vec::new();
    for window_start in 0..=char_starts.len() - 1 - n {
        let start = char_starts[window_start];
        let end = char_starts[window_start + n];
        keys.push(text.as_bytes()[start..end].to_vec());
    }
    keys
}

fn tokenize_ascii_lower(bytes: &[u8]) -> Vec<TokenSpan> {
    let mut tokens = Vec::new();
    let mut cursor = 0_usize;
    while cursor < bytes.len() {
        while cursor < bytes.len() && !is_token_byte(bytes[cursor]) {
            cursor += 1;
        }
        let start = cursor;
        let mut key = Vec::new();
        while cursor < bytes.len() && is_token_byte(bytes[cursor]) {
            key.push(bytes[cursor].to_ascii_lowercase());
            cursor += 1;
        }
        if start < cursor {
            tokens.push(TokenSpan {
                key,
                start,
                end: cursor,
            });
        }
    }
    tokens
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

pub(crate) fn key_hash(key: &[u8]) -> [u8; 16] {
    let hash = blake3::hash(key);
    let mut output = [0_u8; 16];
    output.copy_from_slice(&hash.as_bytes()[..16]);
    output
}

#[allow(clippy::cast_possible_truncation)] // value ranges guaranteed by the loop invariants
fn write_varuint(mut value: u64, output: &mut Vec<u8>) {
    while value >= 0x80 {
        output.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

fn read_varuint(bytes: &[u8], cursor: &mut usize) -> Result<u64> {
    let start = *cursor;
    let mut value = 0_u64;
    let mut shift = 0_u32;

    loop {
        let byte = *bytes.get(*cursor).ok_or(QztError::UnexpectedEof)?;
        *cursor += 1;
        value |= u64::from(byte & 0x7f)
            .checked_shl(shift)
            .ok_or(QztError::ContainerCorrupt)?;

        if byte & 0x80 == 0 {
            let mut minimal = Vec::new();
            write_varuint(value, &mut minimal);
            if minimal.as_slice() != &bytes[start..*cursor] {
                return Err(QztError::ContainerCorrupt);
            }
            return Ok(value);
        }

        shift += 7;
        if shift > 63 {
            return Err(QztError::ContainerCorrupt);
        }
    }
}

pub(crate) fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1_000.0
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenSpan {
    pub(crate) key: Vec<u8>,
    pub(crate) start: usize,
    pub(crate) end: usize,
}
