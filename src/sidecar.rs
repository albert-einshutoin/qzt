use crate::cbor::{encode_deterministic, validate_deterministic, CborValue};
use crate::error::{QztError, Result};
use crate::format::FOOTER_TRAILER_LEN;
use crate::reader::QztReader;
use crate::schema::Checksum;
use crate::search::{
    decode_delta_varint_u64, encode_delta_varint_u64, NgramIndexBuildOptions, RawNgramIndex,
    RawTokenIndex, SearchGranule, SearchOptions, SearchReport, TermDictionaryEntry,
};
use crate::skeleton::open_skeleton_details;

const SIDECAR_MAGIC: &[u8; 8] = b"QZISIDE1";
const HEADER_LEN: usize = 16;

/// Search sidecar index kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    let details = open_skeleton_details(qzt_bytes)?;
    let footer_checksum = qzt_footer_checksum(qzt_bytes, details.footer_payload_offset)?;

    let (index_type, ngram_n, complete, high_df_per_million, granules, terms, postings) = match kind
    {
        SidecarIndexKind::Token => {
            let index = RawTokenIndex::build_from_container(
                qzt_bytes,
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
            let index = RawNgramIndex::build_from_container(
                qzt_bytes,
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

    let terms_offset =
        u64::try_from(granule_bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
    let postings_offset = terms_offset
        .checked_add(u64::try_from(term_bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?)
        .ok_or(QztError::ResourceLimitExceeded)?;
    let index_size_bytes = postings_offset
        .checked_add(
            u64::try_from(posting_bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?,
        )
        .ok_or(QztError::ResourceLimitExceeded)?;

    let manifest = SidecarManifest {
        source_container_id: details.summary.container_id,
        source_original_checksum: details.metadata.original_checksum,
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
            size: u64::try_from(term_bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?,
            checksum: Checksum::blake3(&term_bytes),
        },
        postings: SectionRef {
            offset: postings_offset,
            size: u64::try_from(posting_bytes.len())
                .map_err(|_| QztError::ResourceLimitExceeded)?,
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
    bytes.extend_from_slice(
        &u64::try_from(manifest_bytes.len())
            .map_err(|_| QztError::ResourceLimitExceeded)?
            .to_le_bytes(),
    );
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
        let manifest_size_usize =
            usize::try_from(manifest_size).map_err(|_| QztError::ResourceLimitExceeded)?;
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
                .map(|value| CborValue::Integer(value as i128))
                .unwrap_or(CborValue::Null),
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
    let map = as_map(&value)?;
    if required_text(map, "schema")? != "qzt.sidecar.v1" {
        return Err(QztError::ContainerCorrupt);
    }
    let source_container_id = required_bstr16(map, "source_container_id")?;
    let source_original_checksum = required_checksum(map, "source_original_checksum")?;
    let source_qzt_footer_checksum = required_checksum(map, "source_qzt_footer_checksum")?;
    let index_type = required_text(map, "index_type")?;
    let ngram_n = match required_value(map, "ngram_n")? {
        CborValue::Null => None,
        CborValue::Integer(value) if *value >= 0 => {
            Some(usize::try_from(*value).map_err(|_| QztError::ResourceLimitExceeded)?)
        }
        _ => return Err(QztError::ContainerCorrupt),
    };
    let complete = required_bool(map, "complete")?;
    let high_df_per_million = u32::try_from(required_u64(map, "high_df_per_million")?)
        .map_err(|_| QztError::ResourceLimitExceeded)?;
    let index_manifest = as_map(required_value(map, "index_manifest")?)?;
    let index_size_bytes = required_u64(index_manifest, "index_size_bytes")?;
    let source_size_bytes = required_u64(index_manifest, "source_size_bytes")?;
    let sections = as_map(required_value(map, "sections")?)?;

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
        granules: required_section(sections, "granules")?,
        terms: required_section(sections, "terms")?,
        postings: required_section(sections, "postings")?,
    })
}

fn section_ref_value(section: &SectionRef) -> CborValue {
    CborValue::Map(vec![
        text_pair("offset", CborValue::Integer(i128::from(section.offset))),
        text_pair("size", CborValue::Integer(i128::from(section.size))),
        text_pair("checksum", checksum_value(&section.checksum)),
    ])
}

fn checksum_value(checksum: &Checksum) -> CborValue {
    CborValue::Map(vec![
        text_pair("algorithm", CborValue::Text(checksum.algorithm.clone())),
        text_pair("value", CborValue::Bytes(checksum.value.to_vec())),
    ])
}

fn text_pair(key: &str, value: CborValue) -> (CborValue, CborValue) {
    (CborValue::Text(key.to_owned()), value)
}

fn required_section(map: &[(CborValue, CborValue)], key: &str) -> Result<SectionRef> {
    let section = as_map(required_value(map, key)?)?;
    Ok(SectionRef {
        offset: required_u64(section, "offset")?,
        size: required_u64(section, "size")?,
        checksum: required_checksum(section, "checksum")?,
    })
}

fn required_checksum(map: &[(CborValue, CborValue)], key: &str) -> Result<Checksum> {
    let checksum = as_map(required_value(map, key)?)?;
    let algorithm = required_text(checksum, "algorithm")?;
    if algorithm != "blake3" {
        return Err(QztError::ContainerCorrupt);
    }
    Ok(Checksum {
        algorithm,
        value: required_bstr32(checksum, "value")?,
    })
}

fn required_value<'a>(map: &'a [(CborValue, CborValue)], key: &str) -> Result<&'a CborValue> {
    map.iter()
        .find_map(|(candidate, value)| {
            (candidate == &CborValue::Text(key.to_owned())).then_some(value)
        })
        .ok_or(QztError::ContainerCorrupt)
}

fn as_map(value: &CborValue) -> Result<&[(CborValue, CborValue)]> {
    match value {
        CborValue::Map(entries) => Ok(entries),
        _ => Err(QztError::ContainerCorrupt),
    }
}

fn required_text(map: &[(CborValue, CborValue)], key: &str) -> Result<String> {
    match required_value(map, key)? {
        CborValue::Text(value) => Ok(value.clone()),
        _ => Err(QztError::ContainerCorrupt),
    }
}

fn required_bool(map: &[(CborValue, CborValue)], key: &str) -> Result<bool> {
    match required_value(map, key)? {
        CborValue::Bool(value) => Ok(*value),
        _ => Err(QztError::ContainerCorrupt),
    }
}

fn required_u64(map: &[(CborValue, CborValue)], key: &str) -> Result<u64> {
    match required_value(map, key)? {
        CborValue::Integer(value) if *value >= 0 => {
            u64::try_from(*value).map_err(|_| QztError::ResourceLimitExceeded)
        }
        _ => Err(QztError::ContainerCorrupt),
    }
}

fn required_bstr16(map: &[(CborValue, CborValue)], key: &str) -> Result<[u8; 16]> {
    match required_value(map, key)? {
        CborValue::Bytes(bytes) if bytes.len() == 16 => {
            let mut output = [0_u8; 16];
            output.copy_from_slice(bytes);
            Ok(output)
        }
        _ => Err(QztError::ContainerCorrupt),
    }
}

fn required_bstr32(map: &[(CborValue, CborValue)], key: &str) -> Result<[u8; 32]> {
    match required_value(map, key)? {
        CborValue::Bytes(bytes) if bytes.len() == 32 => {
            let mut output = [0_u8; 32];
            output.copy_from_slice(bytes);
            Ok(output)
        }
        _ => Err(QztError::ContainerCorrupt),
    }
}

fn section_slice<'a>(
    bytes: &'a [u8],
    section_base: usize,
    section: &SectionRef,
) -> Result<&'a [u8]> {
    let start = section_base
        .checked_add(usize::try_from(section.offset).map_err(|_| QztError::ResourceLimitExceeded)?)
        .ok_or(QztError::ResourceLimitExceeded)?;
    let end = start
        .checked_add(usize::try_from(section.size).map_err(|_| QztError::ResourceLimitExceeded)?)
        .ok_or(QztError::ResourceLimitExceeded)?;
    let slice = bytes.get(start..end).ok_or(QztError::UnexpectedEof)?;
    if Checksum::blake3(slice) != section.checksum {
        return Err(QztError::ContainerCorrupt);
    }
    Ok(slice)
}

fn qzt_footer_checksum(qzt_bytes: &[u8], footer_payload_offset: u64) -> Result<Checksum> {
    let start =
        usize::try_from(footer_payload_offset).map_err(|_| QztError::ResourceLimitExceeded)?;
    let end = qzt_bytes
        .len()
        .checked_sub(FOOTER_TRAILER_LEN)
        .ok_or(QztError::InvalidFooterTrailer)?;
    let footer = qzt_bytes.get(start..end).ok_or(QztError::UnexpectedEof)?;
    Ok(Checksum::blake3(footer))
}

fn encode_granules(granules: &[SearchGranule]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    write_u64(
        u64::try_from(granules.len()).map_err(|_| QztError::ResourceLimitExceeded)?,
        &mut bytes,
    );
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
    let mut granules =
        Vec::with_capacity(usize::try_from(count).map_err(|_| QztError::ResourceLimitExceeded)?);
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
    write_u64(
        u64::try_from(terms.len()).map_err(|_| QztError::ResourceLimitExceeded)?,
        &mut bytes,
    );
    for term in terms {
        write_u64(
            u64::try_from(term.key.len()).map_err(|_| QztError::ResourceLimitExceeded)?,
            &mut bytes,
        );
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
    let mut terms =
        Vec::with_capacity(usize::try_from(count).map_err(|_| QztError::ResourceLimitExceeded)?);
    for _ in 0..count {
        let key_len = usize::try_from(read_u64_cursor(bytes, &mut cursor)?)
            .map_err(|_| QztError::ResourceLimitExceeded)?;
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
        let start =
            usize::try_from(term.posting_offset).map_err(|_| QztError::ResourceLimitExceeded)?;
        let end = start
            .checked_add(
                usize::try_from(term.posting_size).map_err(|_| QztError::ResourceLimitExceeded)?,
            )
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
