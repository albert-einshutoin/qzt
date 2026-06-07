use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::chunk_table::ChunkEntry;
use crate::error::{QztError, Result};
use crate::primitives::checked_logical_end;
use crate::reader::QztReader;
use crate::skeleton::open_skeleton_details;

/// Search index source text model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Runtime search limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchOptions {
    pub max_candidate_granules: u64,
    pub max_decoded_bytes: u64,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_candidate_granules: 10_000,
            max_decoded_bytes: 256 * 1024 * 1024,
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
    pub verified_matches: u64,
    pub query_time_ms: f64,
}

/// Search result plus metrics. `capped` means limits stopped verification.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchReport {
    pub hits: Vec<SearchHit>,
    pub metrics: SearchMetrics,
    pub capped: bool,
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
        if options.source == SearchIndexSource::NormalizedUtf8 {
            return Err(QztError::NotImplemented("normalized_utf8 token index"));
        }

        let details = open_skeleton_details(bytes)?;
        let reader = QztReader::open(bytes)?;
        let original = reader.export_all()?;
        std::str::from_utf8(&original).map_err(|_| QztError::InvalidUtf8)?;

        let granules = match options.posting_granularity {
            PostingGranularity::Line => build_line_granules(&original, &details.chunk_entries)?,
        };
        let (terms, postings) = build_term_dictionary(&original, &granules)?;
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
            term.granule_frequency =
                u64::try_from(posting_list.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
            term.posting_offset = posting_offset;
            term.posting_size =
                u64::try_from(encoded.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
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
        let started = Instant::now();
        let query_keys = unique_query_keys(query.as_bytes());
        let mut metrics = self.empty_metrics(query);
        metrics.term_lookups =
            u64::try_from(query_keys.len()).map_err(|_| QztError::ResourceLimitExceeded)?;

        if query_keys.is_empty() {
            metrics.query_time_ms = elapsed_ms(started);
            return Ok(SearchReport {
                hits: Vec::new(),
                metrics,
                capped: false,
            });
        }

        let mut posting_indexes = Vec::with_capacity(query_keys.len());
        for key in &query_keys {
            let Some(term_index) = self.term_index_for_key(key) else {
                metrics.query_time_ms = elapsed_ms(started);
                return Ok(SearchReport {
                    hits: Vec::new(),
                    metrics,
                    capped: false,
                });
            };
            metrics.posting_bytes_read = metrics
                .posting_bytes_read
                .checked_add(self.terms[term_index].posting_size)
                .ok_or(QztError::ResourceLimitExceeded)?;
            posting_indexes.push(term_index);
        }

        posting_indexes.sort_by_key(|index| self.postings[*index].len());
        let posting_refs = posting_indexes
            .iter()
            .map(|index| self.postings[*index].as_slice())
            .collect::<Vec<_>>();
        let candidates = intersect_postings(&posting_refs);
        metrics.candidate_granules =
            u64::try_from(candidates.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
        metrics.candidate_chunks = count_candidate_chunks(&self.granules, &candidates)?;

        if metrics.candidate_granules > options.max_candidate_granules {
            metrics.query_time_ms = elapsed_ms(started);
            return Ok(SearchReport {
                hits: Vec::new(),
                metrics,
                capped: true,
            });
        }

        let mut hits = Vec::new();
        let mut capped = false;
        for granule_id in candidates {
            let granule_index =
                usize::try_from(granule_id).map_err(|_| QztError::ResourceLimitExceeded)?;
            let granule = self
                .granules
                .get(granule_index)
                .ok_or(QztError::ContainerCorrupt)?;
            let next_decoded = metrics
                .decoded_bytes
                .checked_add(granule.byte_length)
                .ok_or(QztError::ResourceLimitExceeded)?;
            if next_decoded > options.max_decoded_bytes {
                capped = true;
                break;
            }

            let decoded = reader.read_range(granule.logical_offset, granule.byte_length)?;
            metrics.decoded_bytes = next_decoded;
            for span in verified_spans(&decoded, &query_keys) {
                let span_offset =
                    u64::try_from(span.start).map_err(|_| QztError::ResourceLimitExceeded)?;
                let span_len = u64::try_from(span.end - span.start)
                    .map_err(|_| QztError::ResourceLimitExceeded)?;
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
            }
        }

        metrics.verified_matches =
            u64::try_from(hits.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
        metrics.query_time_ms = elapsed_ms(started);
        Ok(SearchReport {
            hits,
            metrics,
            capped,
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

fn build_line_granules(input: &[u8], chunk_entries: &[ChunkEntry]) -> Result<Vec<SearchGranule>> {
    let starts = line_starts(input);
    let mut granules = Vec::with_capacity(starts.len());
    for (line_index, start) in starts.iter().enumerate() {
        let end = starts.get(line_index + 1).copied().unwrap_or(input.len());
        let logical_offset = u64::try_from(*start).map_err(|_| QztError::ResourceLimitExceeded)?;
        let byte_length =
            u64::try_from(end - start).map_err(|_| QztError::ResourceLimitExceeded)?;
        let (chunk_start, chunk_end) = chunk_range_for(chunk_entries, logical_offset, byte_length)?;
        granules.push(SearchGranule {
            granule_id: u64::try_from(line_index).map_err(|_| QztError::ResourceLimitExceeded)?,
            logical_offset,
            byte_length,
            chunk_start,
            chunk_end,
            first_line: Some(
                u64::try_from(line_index).map_err(|_| QztError::ResourceLimitExceeded)?,
            ),
            line_count: Some(1),
        });
    }
    Ok(granules)
}

fn build_term_dictionary(
    input: &[u8],
    granules: &[SearchGranule],
) -> Result<(Vec<TermDictionaryEntry>, Vec<Vec<u64>>)> {
    let mut postings_by_key: BTreeMap<Vec<u8>, BTreeSet<u64>> = BTreeMap::new();
    for granule in granules {
        let start =
            usize::try_from(granule.logical_offset).map_err(|_| QztError::ResourceLimitExceeded)?;
        let end = usize::try_from(checked_logical_end(
            granule.logical_offset,
            granule.byte_length,
        )?)
        .map_err(|_| QztError::ResourceLimitExceeded)?;
        let bytes = input.get(start..end).ok_or(QztError::ContainerCorrupt)?;
        for token in tokenize_ascii_lower(bytes) {
            postings_by_key
                .entry(token.key)
                .or_default()
                .insert(granule.granule_id);
        }
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
    Ok((terms, postings))
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
            let granule_index =
                usize::try_from(*granule_id).map_err(|_| QztError::ResourceLimitExceeded)?;
            if granule_index >= granule_count {
                return Err(QztError::ContainerCorrupt);
            }
        }
    }
    Ok(())
}

fn chunk_range_for(chunk_entries: &[ChunkEntry], offset: u64, length: u64) -> Result<(u64, u64)> {
    if length == 0 {
        return Ok((0, 0));
    }
    let end = checked_logical_end(offset, length)?;
    let mut first = None;
    let mut last_exclusive = None;
    for entry in chunk_entries {
        let chunk_end = checked_logical_end(entry.logical_offset, entry.uncompressed_size)?;
        if chunk_end > offset && entry.logical_offset < end {
            first.get_or_insert(entry.chunk_id);
            last_exclusive = Some(
                entry
                    .chunk_id
                    .checked_add(1)
                    .ok_or(QztError::ChunkTableInvalid)?,
            );
        }
    }
    match (first, last_exclusive) {
        (Some(first), Some(last_exclusive)) => Ok((first, last_exclusive)),
        _ => Err(QztError::ChunkTableInvalid),
    }
}

fn count_candidate_chunks(granules: &[SearchGranule], candidates: &[u64]) -> Result<u64> {
    let mut chunks = BTreeSet::new();
    for granule_id in candidates {
        let granule_index =
            usize::try_from(*granule_id).map_err(|_| QztError::ResourceLimitExceeded)?;
        let granule = granules
            .get(granule_index)
            .ok_or(QztError::ContainerCorrupt)?;
        for chunk_id in granule.chunk_start..granule.chunk_end {
            chunks.insert(chunk_id);
        }
    }
    u64::try_from(chunks.len()).map_err(|_| QztError::ResourceLimitExceeded)
}

fn intersect_postings(posting_lists: &[&[u64]]) -> Vec<u64> {
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

fn verified_spans(bytes: &[u8], query_keys: &[Vec<u8>]) -> Vec<TokenSpan> {
    let tokens = tokenize_ascii_lower(bytes);
    if query_keys
        .iter()
        .all(|key| tokens.iter().any(|token| token.key == *key))
    {
        tokens
            .into_iter()
            .filter(|token| token.key == query_keys[0])
            .collect()
    } else {
        Vec::new()
    }
}

fn unique_query_keys(query: &[u8]) -> Vec<Vec<u8>> {
    let mut keys = tokenize_ascii_lower(query)
        .into_iter()
        .map(|token| token.key)
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
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

fn line_starts(input: &[u8]) -> Vec<usize> {
    if input.is_empty() {
        return Vec::new();
    }
    let mut starts = vec![0];
    for index in 0..input.len() {
        if input[index] == b'\n' && index + 1 < input.len() {
            starts.push(index + 1);
        }
    }
    starts
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

fn key_hash(key: &[u8]) -> [u8; 16] {
    let hash = blake3::hash(key);
    let mut output = [0_u8; 16];
    output.copy_from_slice(&hash.as_bytes()[..16]);
    output
}

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

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1_000.0
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TokenSpan {
    key: Vec<u8>,
    start: usize,
    end: usize,
}
