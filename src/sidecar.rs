use std::fs::File;
use std::path::Path;
use std::time::Instant;

use crate::cbor::{encode_deterministic, validate_deterministic, CborValue};
use crate::error::{QztError, Result};
use crate::format::FOOTER_TRAILER_LEN;
use crate::io::ReadAt;
use crate::primitives::{u64_to_usize, usize_to_u64};
use crate::reader::{QztFileReader, QztReader};
use crate::schema::{
    as_map, checksum_value, field, required_bool, required_bstr16, required_checksum,
    required_text, required_u64_with_overflow, text_pair, Checksum,
};
use crate::search::{
    decode_delta_varint_u64, elapsed_ms, encode_delta_varint_u64, intersect_postings, key_hash,
    ngram_keys_for_query, substring_spans, unique_query_keys, verified_spans, verify_candidates,
    NgramIndexBuildOptions, PlannerDecision, RawNgramIndex, RawTokenIndex, SearchGranule,
    SearchMetrics, SearchOptions, SearchReport, TermDictionaryEntry,
};
use crate::skeleton::open_skeleton_details;

const SIDECAR_MAGIC: &[u8; 8] = b"QZISIDE1";
const HEADER_LEN: usize = 16;
const GRANULE_RECORD_LEN: u64 = 56;
const SECTION_HASH_BUFFER: usize = 64 * 1024;

/// Search sidecar index kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SidecarIndexKind {
    Token,
    Ngram { n: usize },
}

/// Opened QZI sidecar.
#[derive(Debug, Clone)]
pub struct QziSidecar {
    pub manifest: SidecarManifest,
    pub index: SidecarSearchIndex,
}

/// Search index restored from the sidecar payload sections.
#[derive(Debug, Clone)]
pub enum SidecarSearchIndex {
    Token(RawTokenIndex),
    Ngram(RawNgramIndex),
}

/// Minimal sidecar manifest fields needed for source validation and lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarManifest {
    pub source_container_id: [u8; 16],
    pub source_original_checksum: Checksum,
    pub source_qzt_footer_checksum: Checksum,
    pub index_type: String,
    pub ngram_n: Option<usize>,
    pub complete: bool,
    pub high_df_per_million: u32,
    pub index_size_bytes: u64,
    pub source_size_bytes: u64,
    granules: SectionRef,
    terms: SectionRef,
    postings: SectionRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SectionRef {
    offset: u64,
    size: u64,
    checksum: Checksum,
}

pub fn build_search_sidecar(qzt_bytes: &[u8], kind: SidecarIndexKind) -> Result<Vec<u8>> {
    let len = usize_to_u64(qzt_bytes.len())?;
    let reader = QztFileReader::open_read_at(qzt_bytes, len)?;
    build_search_sidecar_from_file(&reader, kind)
}

/// Builds a QZI sidecar from a file-backed container, decoding one chunk at a
/// time instead of materializing the full original text. Produces bytes
/// identical to [`build_search_sidecar`].
pub fn build_search_sidecar_from_file<R: ReadAt>(
    reader: &QztFileReader<R>,
    kind: SidecarIndexKind,
) -> Result<Vec<u8>> {
    let details = reader.skeleton_details();
    let footer_checksum = reader.footer_checksum()?;

    let (index_type, ngram_n, complete, high_df_per_million, granules, terms, postings) = match kind
    {
        SidecarIndexKind::Token => {
            let index = RawTokenIndex::build_from_file(
                reader,
                crate::search::TokenIndexBuildOptions::default(),
            )?;
            (
                "token".to_owned(),
                None,
                index.complete,
                200_000,
                index.granules,
                index.terms,
                index.postings,
            )
        }
        SidecarIndexKind::Ngram { n } => {
            let index = RawNgramIndex::build_from_file(
                reader,
                NgramIndexBuildOptions {
                    n,
                    ..NgramIndexBuildOptions::default()
                },
            )?;
            (
                "ngram".to_owned(),
                Some(n),
                index.complete,
                index.planner_config.high_df_per_million,
                index.granules,
                index.terms,
                index.postings,
            )
        }
    };

    let granule_bytes = encode_granules(&granules)?;
    let posting_bytes = encode_posting_section(&postings)?;
    let term_bytes = encode_terms(&terms)?;

    let terms_offset = usize_to_u64(granule_bytes.len())?;
    let postings_offset = terms_offset
        .checked_add(usize_to_u64(term_bytes.len())?)
        .ok_or(QztError::ResourceLimitExceeded)?;
    let index_size_bytes = postings_offset
        .checked_add(usize_to_u64(posting_bytes.len())?)
        .ok_or(QztError::ResourceLimitExceeded)?;

    let manifest = SidecarManifest {
        source_container_id: details.summary.container_id,
        source_original_checksum: details.metadata.original_checksum.clone(),
        source_qzt_footer_checksum: footer_checksum,
        index_type,
        ngram_n,
        complete,
        high_df_per_million,
        index_size_bytes,
        source_size_bytes: details.summary.original_size,
        granules: SectionRef {
            offset: 0,
            size: terms_offset,
            checksum: Checksum::blake3(&granule_bytes),
        },
        terms: SectionRef {
            offset: terms_offset,
            size: usize_to_u64(term_bytes.len())?,
            checksum: Checksum::blake3(&term_bytes),
        },
        postings: SectionRef {
            offset: postings_offset,
            size: usize_to_u64(posting_bytes.len())?,
            checksum: Checksum::blake3(&posting_bytes),
        },
    };
    let manifest_bytes = encode_manifest(&manifest)?;

    let mut bytes = Vec::with_capacity(
        HEADER_LEN
            + manifest_bytes.len()
            + granule_bytes.len()
            + term_bytes.len()
            + posting_bytes.len(),
    );
    bytes.extend_from_slice(SIDECAR_MAGIC);
    bytes.extend_from_slice(&usize_to_u64(manifest_bytes.len())?.to_le_bytes());
    bytes.extend_from_slice(&manifest_bytes);
    bytes.extend_from_slice(&granule_bytes);
    bytes.extend_from_slice(&term_bytes);
    bytes.extend_from_slice(&posting_bytes);
    Ok(bytes)
}

impl QziSidecar {
    pub fn open(qzt_bytes: &[u8], sidecar_bytes: &[u8]) -> Result<Self> {
        if sidecar_bytes.len() < HEADER_LEN || &sidecar_bytes[..8] != SIDECAR_MAGIC {
            return Err(QztError::InvalidHeader);
        }

        let manifest_size = read_u64_le(&sidecar_bytes[8..16])?;
        let manifest_size_usize = u64_to_usize(manifest_size)?;
        let manifest_end = HEADER_LEN
            .checked_add(manifest_size_usize)
            .ok_or(QztError::ResourceLimitExceeded)?;
        let manifest_bytes = sidecar_bytes
            .get(HEADER_LEN..manifest_end)
            .ok_or(QztError::UnexpectedEof)?;
        let manifest = decode_manifest(manifest_bytes)?;

        let details = open_skeleton_details(qzt_bytes)?;
        if manifest.source_container_id != details.summary.container_id {
            return Err(QztError::ContainerIdMismatch);
        }
        if manifest.source_original_checksum != details.metadata.original_checksum {
            return Err(QztError::ContainerCorrupt);
        }
        if manifest.source_qzt_footer_checksum
            != qzt_footer_checksum(qzt_bytes, details.footer_payload_offset)?
        {
            return Err(QztError::ContainerCorrupt);
        }

        let section_base = manifest_end;
        let granule_bytes = section_slice(sidecar_bytes, section_base, &manifest.granules)?;
        let term_bytes = section_slice(sidecar_bytes, section_base, &manifest.terms)?;
        let posting_bytes = section_slice(sidecar_bytes, section_base, &manifest.postings)?;
        let granules = decode_granules(granule_bytes)?;
        let terms = decode_terms(term_bytes)?;
        let postings = decode_posting_section(posting_bytes, &terms)?;

        let index = match manifest.index_type.as_str() {
            "token" => SidecarSearchIndex::Token(RawTokenIndex::from_parts(
                manifest.source_container_id,
                manifest.source_size_bytes,
                granules,
                terms,
                postings,
            )?),
            "ngram" => {
                let n = manifest.ngram_n.ok_or(QztError::ContainerCorrupt)?;
                SidecarSearchIndex::Ngram(RawNgramIndex::from_parts(
                    manifest.source_container_id,
                    manifest.source_size_bytes,
                    granules,
                    terms,
                    postings,
                    NgramIndexBuildOptions {
                        n,
                        complete: manifest.complete,
                        high_df_per_million: manifest.high_df_per_million,
                        ..NgramIndexBuildOptions::default()
                    },
                )?)
            }
            _ => return Err(QztError::ContainerCorrupt),
        };

        Ok(Self { manifest, index })
    }

    pub fn search(
        &self,
        reader: &QztReader,
        query: &str,
        options: SearchOptions,
    ) -> Result<SearchReport> {
        match &self.index {
            SidecarSearchIndex::Token(index) => index.search(reader, query, options),
            SidecarSearchIndex::Ngram(index) => index.search(reader, query, options),
        }
    }
}

/// File-backed QZI sidecar with lazy posting and granule lookup.
///
/// Opening loads only the manifest and the term dictionary into memory and
/// stream-verifies all section checksums with a bounded buffer. Posting lists
/// and granule records are fetched from the source per query, so search
/// memory scales with the query's candidate set instead of the sidecar size.
///
/// Reported metrics differ from the in-memory [`QziSidecar`] in two
/// deliberate ways: `posting_bytes_read` counts the bytes actually fetched
/// (no skip-probe simulation), and `candidate_chunks` stays `0` when the
/// candidate cap rejects the query before granule records are fetched.
pub struct QziFileSidecar<R> {
    manifest: SidecarManifest,
    source: R,
    section_base: u64,
    granule_count: u64,
    terms: Vec<TermDictionaryEntry>,
}

impl<R: ReadAt> QziFileSidecar<R> {
    /// Opens a sidecar over a positioned source and binds it to `container`.
    pub fn open_read_at<C: ReadAt>(
        source: R,
        len: u64,
        container: &QztFileReader<C>,
    ) -> Result<Self> {
        let mut header = [0_u8; HEADER_LEN];
        if len < HEADER_LEN as u64 {
            return Err(QztError::InvalidHeader);
        }
        source
            .read_exact_at(0, &mut header)
            .map_err(|e| map_read_error(&e))?;
        if &header[..8] != SIDECAR_MAGIC {
            return Err(QztError::InvalidHeader);
        }

        let manifest_size = read_u64_le(&header[8..16])?;
        let manifest_end = (HEADER_LEN as u64)
            .checked_add(manifest_size)
            .ok_or(QztError::ResourceLimitExceeded)?;
        if manifest_end > len {
            return Err(QztError::UnexpectedEof);
        }
        let manifest_bytes = read_vec(&source, HEADER_LEN as u64, manifest_size)?;
        let manifest = decode_manifest(&manifest_bytes)?;

        let details = container.skeleton_details();
        if manifest.source_container_id != details.summary.container_id {
            return Err(QztError::ContainerIdMismatch);
        }
        if manifest.source_original_checksum != details.metadata.original_checksum {
            return Err(QztError::ContainerCorrupt);
        }
        if manifest.source_qzt_footer_checksum != container.footer_checksum()? {
            return Err(QztError::ContainerCorrupt);
        }
        match manifest.index_type.as_str() {
            "token" => {}
            "ngram" => {
                let n = manifest.ngram_n.ok_or(QztError::ContainerCorrupt)?;
                if n == 0 {
                    return Err(QztError::ContainerCorrupt);
                }
            }
            _ => return Err(QztError::ContainerCorrupt),
        }

        let section_base = manifest_end;
        for section in [&manifest.granules, &manifest.terms, &manifest.postings] {
            verify_section_checksum(&source, len, section_base, section)?;
        }

        if manifest.granules.size < 8 {
            return Err(QztError::ContainerCorrupt);
        }
        let granule_section_offset = section_base
            .checked_add(manifest.granules.offset)
            .ok_or(QztError::ResourceLimitExceeded)?;
        let count_bytes = read_vec(&source, granule_section_offset, 8)?;
        let granule_count = read_u64_le(&count_bytes)?;
        let expected_granule_size = granule_count
            .checked_mul(GRANULE_RECORD_LEN)
            .and_then(|records| records.checked_add(8))
            .ok_or(QztError::ResourceLimitExceeded)?;
        if expected_granule_size != manifest.granules.size {
            return Err(QztError::ContainerCorrupt);
        }

        let terms_offset = section_base
            .checked_add(manifest.terms.offset)
            .ok_or(QztError::ResourceLimitExceeded)?;
        let term_bytes = read_vec(&source, terms_offset, manifest.terms.size)?;
        let terms = decode_terms(&term_bytes)?;
        for term in &terms {
            let posting_end = term
                .posting_offset
                .checked_add(term.posting_size)
                .ok_or(QztError::ResourceLimitExceeded)?;
            if posting_end > manifest.postings.size {
                return Err(QztError::ContainerCorrupt);
            }
        }

        Ok(Self {
            manifest,
            source,
            section_base,
            granule_count,
            terms,
        })
    }

    /// Validated sidecar manifest.
    pub fn manifest(&self) -> &SidecarManifest {
        &self.manifest
    }

    /// Search over a file-backed container. Fetches only the queried terms'
    /// posting lists and the candidate granule records from the sidecar, and
    /// decodes only candidate chunks from the container.
    pub fn search<C: ReadAt>(
        &self,
        reader: &QztFileReader<C>,
        query: &str,
        options: SearchOptions,
    ) -> Result<SearchReport> {
        let started = Instant::now();
        let is_ngram = self.manifest.index_type == "ngram";
        let index_kind: &'static str = if is_ngram { "ngram" } else { "token" };
        let query_keys = if is_ngram {
            let n = self.manifest.ngram_n.ok_or(QztError::ContainerCorrupt)?;
            ngram_keys_for_query(query, n)?
        } else {
            unique_query_keys(query.as_bytes())
        };

        let mut planner = PlannerDecision::new(query_keys.clone());
        let mut metrics = self.empty_metrics(query, index_kind);
        metrics.term_lookups = usize_to_u64(query_keys.len())?;

        if query_keys.is_empty() {
            metrics.query_time_ms = elapsed_ms(started);
            return Ok(SearchReport {
                hits: Vec::new(),
                metrics,
                capped: false,
                planner,
                incomplete_reason: Some(if is_ngram {
                    "query_shorter_than_ngram_n"
                } else {
                    "query_has_no_indexable_tokens"
                }),
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
                    incomplete_reason: (is_ngram && !self.manifest.complete)
                        .then_some("missing_required_key_in_incomplete_index"),
                });
            };
            metrics.posting_bytes_read = metrics
                .posting_bytes_read
                .checked_add(self.terms[term_index].posting_size)
                .ok_or(QztError::ResourceLimitExceeded)?;
            term_indexes.push(term_index);
        }

        if is_ngram {
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
            }
        } else {
            term_indexes.sort_by_key(|index| self.terms[*index].granule_frequency);
        }
        planner.selected_keys = term_indexes
            .iter()
            .map(|index| self.terms[*index].key.clone())
            .collect();

        let posting_lists = term_indexes
            .iter()
            .map(|index| self.fetch_postings(*index))
            .collect::<Result<Vec<_>>>()?;
        let posting_refs = posting_lists.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let candidates = intersect_postings(&posting_refs);
        metrics.candidate_granules = usize_to_u64(candidates.len())?;

        if metrics.candidate_granules > options.max_candidate_granules
            || options.max_search_results == 0
        {
            metrics.query_time_ms = elapsed_ms(started);
            return Ok(SearchReport {
                hits: Vec::new(),
                metrics,
                capped: true,
                planner,
                incomplete_reason: None,
            });
        }

        let granules = candidates
            .iter()
            .map(|granule_id| self.fetch_granule(*granule_id))
            .collect::<Result<Vec<_>>>()?;
        metrics.candidate_chunks = count_chunks(&granules)?;

        let verification = verify_candidates(
            &candidates,
            &mut |granule_id| {
                let position = candidates
                    .binary_search(&granule_id)
                    .map_err(|_| QztError::ContainerCorrupt)?;
                granules
                    .get(position)
                    .cloned()
                    .ok_or(QztError::ContainerCorrupt)
            },
            &mut |offset, length, cache| reader.read_range_cached(offset, length, cache),
            &mut |decoded| {
                if is_ngram {
                    substring_spans(decoded, query.as_bytes())
                } else {
                    verified_spans(decoded, &query_keys)
                }
            },
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
        let granule_count = u128::from(self.granule_count.max(1));
        let frequency = u128::from(self.terms[term_index].granule_frequency);
        let per_million = frequency.saturating_mul(1_000_000) / granule_count;
        per_million >= u128::from(self.manifest.high_df_per_million)
    }

    fn fetch_postings(&self, term_index: usize) -> Result<Vec<u64>> {
        let term = self
            .terms
            .get(term_index)
            .ok_or(QztError::ContainerCorrupt)?;
        let offset = self
            .section_base
            .checked_add(self.manifest.postings.offset)
            .and_then(|base| base.checked_add(term.posting_offset))
            .ok_or(QztError::ResourceLimitExceeded)?;
        let bytes = read_vec(&self.source, offset, term.posting_size)?;
        let postings = decode_delta_varint_u64(&bytes)?;
        for granule_id in &postings {
            if *granule_id >= self.granule_count {
                return Err(QztError::ContainerCorrupt);
            }
        }
        Ok(postings)
    }

    fn fetch_granule(&self, granule_id: u64) -> Result<SearchGranule> {
        if granule_id >= self.granule_count {
            return Err(QztError::ContainerCorrupt);
        }
        let record_offset = granule_id
            .checked_mul(GRANULE_RECORD_LEN)
            .and_then(|relative| relative.checked_add(8))
            .and_then(|relative| relative.checked_add(self.manifest.granules.offset))
            .and_then(|relative| relative.checked_add(self.section_base))
            .ok_or(QztError::ResourceLimitExceeded)?;
        let bytes = read_vec(&self.source, record_offset, GRANULE_RECORD_LEN)?;

        let mut cursor = 0_usize;
        let granule = SearchGranule {
            granule_id: read_u64_cursor(&bytes, &mut cursor)?,
            logical_offset: read_u64_cursor(&bytes, &mut cursor)?,
            byte_length: read_u64_cursor(&bytes, &mut cursor)?,
            chunk_start: read_u64_cursor(&bytes, &mut cursor)?,
            chunk_end: read_u64_cursor(&bytes, &mut cursor)?,
            first_line: none_if_max(read_u64_cursor(&bytes, &mut cursor)?),
            line_count: none_if_max(read_u64_cursor(&bytes, &mut cursor)?),
        };
        if granule.granule_id != granule_id {
            return Err(QztError::ContainerCorrupt);
        }
        let end = granule
            .logical_offset
            .checked_add(granule.byte_length)
            .ok_or(QztError::LogicalRangeOutOfBounds)?;
        if end > self.manifest.source_size_bytes {
            return Err(QztError::LogicalRangeOutOfBounds);
        }
        if granule.chunk_end < granule.chunk_start {
            return Err(QztError::ChunkTableInvalid);
        }
        Ok(granule)
    }

    fn empty_metrics(&self, query: &str, index_kind: &'static str) -> SearchMetrics {
        let index_size_bytes = self.manifest.index_size_bytes;
        let index_size_ratio = if self.manifest.source_size_bytes == 0 {
            0.0
        } else {
            index_size_bytes as f64 / self.manifest.source_size_bytes as f64
        };

        SearchMetrics {
            query: query.to_owned(),
            index_kind,
            posting_granularity: "line",
            index_size_bytes,
            source_size_bytes: self.manifest.source_size_bytes,
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
}

impl QziFileSidecar<File> {
    /// Opens a sidecar file from a filesystem path and binds it to `container`.
    pub fn open_path<C: ReadAt>(
        path: impl AsRef<Path>,
        container: &QztFileReader<C>,
    ) -> Result<Self> {
        let file = File::open(path).map_err(|error| QztError::Io(error.kind()))?;
        let len = file
            .metadata()
            .map_err(|error| QztError::Io(error.kind()))?
            .len();
        Self::open_read_at(file, len, container)
    }
}

fn count_chunks(granules: &[SearchGranule]) -> Result<u64> {
    let mut chunks = std::collections::BTreeSet::new();
    for granule in granules {
        for chunk_id in granule.chunk_start..granule.chunk_end {
            chunks.insert(chunk_id);
        }
    }
    usize_to_u64(chunks.len())
}

fn map_read_error(error: &std::io::Error) -> QztError {
    match error.kind() {
        std::io::ErrorKind::UnexpectedEof => QztError::UnexpectedEof,
        _ => QztError::ContainerCorrupt,
    }
}

fn read_vec<R: ReadAt>(source: &R, offset: u64, size: u64) -> Result<Vec<u8>> {
    let len = u64_to_usize(size)?;
    let mut bytes = vec![0_u8; len];
    source
        .read_exact_at(offset, &mut bytes)
        .map_err(|e| map_read_error(&e))?;
    Ok(bytes)
}

fn verify_section_checksum<R: ReadAt>(
    source: &R,
    len: u64,
    section_base: u64,
    section: &SectionRef,
) -> Result<()> {
    let start = section_base
        .checked_add(section.offset)
        .ok_or(QztError::ResourceLimitExceeded)?;
    let end = start
        .checked_add(section.size)
        .ok_or(QztError::ResourceLimitExceeded)?;
    if end > len {
        return Err(QztError::UnexpectedEof);
    }

    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; SECTION_HASH_BUFFER];
    let mut offset = start;
    while offset < end {
        let remaining = end - offset;
        let read_len = u64_to_usize(remaining.min(buffer.len() as u64))?;
        source
            .read_exact_at(offset, &mut buffer[..read_len])
            .map_err(|e| map_read_error(&e))?;
        hasher.update(&buffer[..read_len]);
        offset = offset
            .checked_add(read_len as u64)
            .ok_or(QztError::ResourceLimitExceeded)?;
    }
    let actual = Checksum::from_hasher(&hasher);
    if actual != section.checksum {
        return Err(QztError::ContainerCorrupt);
    }
    Ok(())
}

fn encode_manifest(manifest: &SidecarManifest) -> Result<Vec<u8>> {
    encode_deterministic(&CborValue::Map(vec![
        text_pair("schema", CborValue::Text("qzt.sidecar.v1".to_owned())),
        text_pair(
            "source_container_id",
            CborValue::Bytes(manifest.source_container_id.to_vec()),
        ),
        text_pair(
            "source_format_version",
            CborValue::Array(vec![CborValue::Integer(0), CborValue::Integer(1)]),
        ),
        text_pair(
            "source_original_checksum",
            checksum_value(&manifest.source_original_checksum),
        ),
        text_pair(
            "source_qzt_footer_checksum",
            checksum_value(&manifest.source_qzt_footer_checksum),
        ),
        text_pair("index_type", CborValue::Text(manifest.index_type.clone())),
        text_pair(
            "ngram_n",
            manifest
                .ngram_n
                .map_or(CborValue::Null, |value| CborValue::Integer(value as i128)),
        ),
        text_pair("complete", CborValue::Bool(manifest.complete)),
        text_pair(
            "high_df_per_million",
            CborValue::Integer(i128::from(manifest.high_df_per_million)),
        ),
        text_pair(
            "index_manifest",
            CborValue::Map(vec![
                text_pair("schema", CborValue::Text("qzt.search-index.v1".to_owned())),
                text_pair("kind", CborValue::Text(manifest.index_type.clone())),
                text_pair("posting_granularity", CborValue::Text("line".to_owned())),
                text_pair(
                    "index_size_bytes",
                    CborValue::Integer(i128::from(manifest.index_size_bytes)),
                ),
                text_pair(
                    "source_size_bytes",
                    CborValue::Integer(i128::from(manifest.source_size_bytes)),
                ),
            ]),
        ),
        text_pair(
            "sections",
            CborValue::Map(vec![
                text_pair("granules", section_ref_value(&manifest.granules)),
                text_pair("terms", section_ref_value(&manifest.terms)),
                text_pair("postings", section_ref_value(&manifest.postings)),
            ]),
        ),
    ]))
}

fn decode_manifest(bytes: &[u8]) -> Result<SidecarManifest> {
    let value = validate_deterministic(bytes)?;
    let map = as_map(&value, QztError::ContainerCorrupt)?;
    if required_text(map, "schema", QztError::ContainerCorrupt)? != "qzt.sidecar.v1" {
        return Err(QztError::ContainerCorrupt);
    }
    let source_container_id =
        required_bstr16(map, "source_container_id", QztError::ContainerCorrupt)?;
    let source_original_checksum =
        required_checksum(map, "source_original_checksum", QztError::ContainerCorrupt)?;
    let source_qzt_footer_checksum =
        required_checksum(map, "source_qzt_footer_checksum", QztError::ContainerCorrupt)?;
    let index_type = required_text(map, "index_type", QztError::ContainerCorrupt)?;
    let ngram_n = match field(map, "ngram_n", QztError::ContainerCorrupt)? {
        CborValue::Null => None,
        CborValue::Integer(value) if *value >= 0 => Some(
            (*value)
                .try_into()
                .map_err(|_| QztError::ResourceLimitExceeded)?,
        ),
        _ => return Err(QztError::ContainerCorrupt),
    };
    let complete = required_bool(map, "complete", QztError::ContainerCorrupt)?;
    let high_df_per_million = u32::try_from(required_u64_with_overflow(
        map,
        "high_df_per_million",
        QztError::ContainerCorrupt,
        QztError::ResourceLimitExceeded,
    )?)
    .map_err(|_| QztError::ResourceLimitExceeded)?;
    let index_manifest = as_map(
        field(map, "index_manifest", QztError::ContainerCorrupt)?,
        QztError::ContainerCorrupt,
    )?;
    let index_size_bytes = required_u64_with_overflow(
        index_manifest,
        "index_size_bytes",
        QztError::ContainerCorrupt,
        QztError::ResourceLimitExceeded,
    )?;
    let source_size_bytes = required_u64_with_overflow(
        index_manifest,
        "source_size_bytes",
        QztError::ContainerCorrupt,
        QztError::ResourceLimitExceeded,
    )?;
    let sections = as_map(
        field(map, "sections", QztError::ContainerCorrupt)?,
        QztError::ContainerCorrupt,
    )?;

    Ok(SidecarManifest {
        source_container_id,
        source_original_checksum,
        source_qzt_footer_checksum,
        index_type,
        ngram_n,
        complete,
        high_df_per_million,
        index_size_bytes,
        source_size_bytes,
        granules: section_ref_from(sections, "granules")?,
        terms: section_ref_from(sections, "terms")?,
        postings: section_ref_from(sections, "postings")?,
    })
}

fn section_ref_value(section: &SectionRef) -> CborValue {
    CborValue::Map(vec![
        text_pair("offset", CborValue::Integer(i128::from(section.offset))),
        text_pair("size", CborValue::Integer(i128::from(section.size))),
        text_pair("checksum", checksum_value(&section.checksum)),
    ])
}

fn section_ref_from(map: &[(CborValue, CborValue)], key: &str) -> Result<SectionRef> {
    let section = as_map(field(map, key, QztError::ContainerCorrupt)?, QztError::ContainerCorrupt)?;
    Ok(SectionRef {
        offset: required_u64_with_overflow(
            section,
            "offset",
            QztError::ContainerCorrupt,
            QztError::ResourceLimitExceeded,
        )?,
        size: required_u64_with_overflow(
            section,
            "size",
            QztError::ContainerCorrupt,
            QztError::ResourceLimitExceeded,
        )?,
        checksum: required_checksum(section, "checksum", QztError::ContainerCorrupt)?,
    })
}

fn section_slice<'a>(
    bytes: &'a [u8],
    section_base: usize,
    section: &SectionRef,
) -> Result<&'a [u8]> {
    let start = section_base
        .checked_add(u64_to_usize(section.offset)?)
        .ok_or(QztError::ResourceLimitExceeded)?;
    let end = start
        .checked_add(u64_to_usize(section.size)?)
        .ok_or(QztError::ResourceLimitExceeded)?;
    let slice = bytes.get(start..end).ok_or(QztError::UnexpectedEof)?;
    if Checksum::blake3(slice) != section.checksum {
        return Err(QztError::ContainerCorrupt);
    }
    Ok(slice)
}

fn qzt_footer_checksum(qzt_bytes: &[u8], footer_payload_offset: u64) -> Result<Checksum> {
    let start = u64_to_usize(footer_payload_offset)?;
    let end = qzt_bytes
        .len()
        .checked_sub(FOOTER_TRAILER_LEN)
        .ok_or(QztError::InvalidFooterTrailer)?;
    let footer = qzt_bytes.get(start..end).ok_or(QztError::UnexpectedEof)?;
    Ok(Checksum::blake3(footer))
}

fn encode_granules(granules: &[SearchGranule]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    write_u64(usize_to_u64(granules.len())?, &mut bytes);
    for granule in granules {
        write_u64(granule.granule_id, &mut bytes);
        write_u64(granule.logical_offset, &mut bytes);
        write_u64(granule.byte_length, &mut bytes);
        write_u64(granule.chunk_start, &mut bytes);
        write_u64(granule.chunk_end, &mut bytes);
        write_u64(granule.first_line.unwrap_or(u64::MAX), &mut bytes);
        write_u64(granule.line_count.unwrap_or(u64::MAX), &mut bytes);
    }
    Ok(bytes)
}

fn decode_granules(bytes: &[u8]) -> Result<Vec<SearchGranule>> {
    let mut cursor = 0_usize;
    let count = read_u64_cursor(bytes, &mut cursor)?;
    let mut granules = Vec::with_capacity(u64_to_usize(count)?);
    for _ in 0..count {
        let granule_id = read_u64_cursor(bytes, &mut cursor)?;
        let logical_offset = read_u64_cursor(bytes, &mut cursor)?;
        let byte_length = read_u64_cursor(bytes, &mut cursor)?;
        let chunk_start = read_u64_cursor(bytes, &mut cursor)?;
        let chunk_end = read_u64_cursor(bytes, &mut cursor)?;
        let first_line = none_if_max(read_u64_cursor(bytes, &mut cursor)?);
        let line_count = none_if_max(read_u64_cursor(bytes, &mut cursor)?);
        granules.push(SearchGranule {
            granule_id,
            logical_offset,
            byte_length,
            chunk_start,
            chunk_end,
            first_line,
            line_count,
        });
    }
    if cursor != bytes.len() {
        return Err(QztError::ContainerCorrupt);
    }
    Ok(granules)
}

fn encode_terms(terms: &[TermDictionaryEntry]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    write_u64(usize_to_u64(terms.len())?, &mut bytes);
    for term in terms {
        write_u64(usize_to_u64(term.key.len())?, &mut bytes);
        bytes.extend_from_slice(&term.key);
        bytes.extend_from_slice(&term.key_hash);
        write_u64(term.document_frequency, &mut bytes);
        write_u64(term.granule_frequency, &mut bytes);
        write_u64(term.posting_offset, &mut bytes);
        write_u64(term.posting_size, &mut bytes);
        write_u64(term.skip_offset, &mut bytes);
        write_u64(term.skip_size, &mut bytes);
        write_u64(term.flags, &mut bytes);
    }
    Ok(bytes)
}

fn decode_terms(bytes: &[u8]) -> Result<Vec<TermDictionaryEntry>> {
    let mut cursor = 0_usize;
    let count = read_u64_cursor(bytes, &mut cursor)?;
    let mut terms = Vec::with_capacity(u64_to_usize(count)?);
    for _ in 0..count {
        let key_len = u64_to_usize(read_u64_cursor(bytes, &mut cursor)?)?;
        let key = read_exact(bytes, &mut cursor, key_len)?.to_vec();
        let mut key_hash = [0_u8; 16];
        key_hash.copy_from_slice(read_exact(bytes, &mut cursor, 16)?);
        terms.push(TermDictionaryEntry {
            key,
            key_hash,
            document_frequency: read_u64_cursor(bytes, &mut cursor)?,
            granule_frequency: read_u64_cursor(bytes, &mut cursor)?,
            posting_offset: read_u64_cursor(bytes, &mut cursor)?,
            posting_size: read_u64_cursor(bytes, &mut cursor)?,
            skip_offset: read_u64_cursor(bytes, &mut cursor)?,
            skip_size: read_u64_cursor(bytes, &mut cursor)?,
            flags: read_u64_cursor(bytes, &mut cursor)?,
        });
    }
    if cursor != bytes.len() {
        return Err(QztError::ContainerCorrupt);
    }
    Ok(terms)
}

fn encode_posting_section(postings: &[Vec<u64>]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for posting in postings {
        bytes.extend_from_slice(&encode_delta_varint_u64(posting)?);
    }
    Ok(bytes)
}

fn decode_posting_section(bytes: &[u8], terms: &[TermDictionaryEntry]) -> Result<Vec<Vec<u64>>> {
    let mut postings = Vec::with_capacity(terms.len());
    for term in terms {
        let start = u64_to_usize(term.posting_offset)?;
        let end = start
            .checked_add(u64_to_usize(term.posting_size)?)
            .ok_or(QztError::ResourceLimitExceeded)?;
        let encoded = bytes.get(start..end).ok_or(QztError::UnexpectedEof)?;
        postings.push(decode_delta_varint_u64(encoded)?);
    }
    Ok(postings)
}

fn write_u64(value: u64, bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn read_u64_le(bytes: &[u8]) -> Result<u64> {
    let array: [u8; 8] = bytes.try_into().map_err(|_| QztError::UnexpectedEof)?;
    Ok(u64::from_le_bytes(array))
}

fn read_u64_cursor(bytes: &[u8], cursor: &mut usize) -> Result<u64> {
    let value = read_u64_le(read_exact(bytes, cursor, 8)?)?;
    Ok(value)
}

fn read_exact<'a>(bytes: &'a [u8], cursor: &mut usize, len: usize) -> Result<&'a [u8]> {
    let end = cursor
        .checked_add(len)
        .ok_or(QztError::ResourceLimitExceeded)?;
    let slice = bytes.get(*cursor..end).ok_or(QztError::UnexpectedEof)?;
    *cursor = end;
    Ok(slice)
}

fn none_if_max(value: u64) -> Option<u64> {
    (value != u64::MAX).then_some(value)
}

#[cfg(test)]
mod manifest_tests {
    use super::*;

    fn checksum_fixture(label: &[u8]) -> CborValue {
        checksum_value(&Checksum::blake3(label))
    }

    fn section_fixture() -> CborValue {
        CborValue::Map(vec![
            text_pair("offset", CborValue::Integer(0)),
            text_pair("size", CborValue::Integer(0)),
            text_pair("checksum", checksum_fixture(b"section")),
        ])
    }

    fn manifest_fixture(high_df_per_million: CborValue) -> CborValue {
        CborValue::Map(vec![
            text_pair("schema", CborValue::Text("qzt.sidecar.v1".to_owned())),
            text_pair("source_container_id", CborValue::Bytes(vec![0; 16])),
            text_pair("source_original_checksum", checksum_fixture(b"source")),
            text_pair("source_qzt_footer_checksum", checksum_fixture(b"footer")),
            text_pair("index_type", CborValue::Text("token".to_owned())),
            text_pair("ngram_n", CborValue::Null),
            text_pair("complete", CborValue::Bool(true)),
            text_pair("high_df_per_million", high_df_per_million),
            text_pair(
                "index_manifest",
                CborValue::Map(vec![
                    text_pair("index_size_bytes", CborValue::Integer(0)),
                    text_pair("source_size_bytes", CborValue::Integer(0)),
                ]),
            ),
            text_pair(
                "sections",
                CborValue::Map(vec![
                    text_pair("granules", section_fixture()),
                    text_pair("terms", section_fixture()),
                    text_pair("postings", section_fixture()),
                ]),
            ),
        ])
    }

    #[test]
    fn manifest_u32_overflow_preserves_resource_limit_error() {
        let overflow = CborValue::Integer(i128::from(u32::MAX) + 1);
        let bytes = encode_deterministic(&manifest_fixture(overflow)).expect("manifest encodes");
        let err = decode_manifest(&bytes).expect_err("oversized u32 is rejected");
        assert!(matches!(err, QztError::ResourceLimitExceeded));
    }
}
