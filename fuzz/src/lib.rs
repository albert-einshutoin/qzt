//! Bounded, replayable entry points for QZI and DLI fuzzing.

use std::sync::OnceLock;

use qzt::chunk_table::ChunkEntry;
use qzt::chunker::plan_chunks;
use qzt::dense_line_index::{DenseLineEntry, DenseLineIndex};
use qzt::sidecar::{legacy_sidecar_for_testing, mutate_sidecar_section_for_testing};
use qzt::{
    ChunkerOptions, QziFileSidecar, QziSidecar, QztError, QztFileReader, QztReader, SearchOptions,
    SearchReport, SidecarIndexKind, SidecarLimits, WriterOptions, build_search_sidecar, pack_bytes,
};

const SOURCES: [&[u8]; 8] = [
    b"alpha alpha beta\n",
    b"alpha\r\nbeta\r\n",
    "αalpha🙂 alpha\n".as_bytes(),
    b"alpha beta\nalpha gamma\n",
    b"",
    b"aaaa\n",
    b"alpha alpha beta alpha beta\n",
    b"a\nb\n",
];

fn options() -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size: 16,
            max_chunk_size: 16,
        },
        ..WriterOptions::default()
    }
}

struct Fixture {
    source: &'static [u8],
    qzt: Vec<u8>,
    qzi: Vec<u8>,
    ngram: bool,
    legacy: bool,
}

fn fixtures() -> &'static [Fixture] {
    static FIXTURES: OnceLock<Vec<Fixture>> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let mut fixtures = Vec::with_capacity(SOURCES.len() * 4);
        for source in SOURCES {
            let qzt = pack_bytes(source, options()).expect("bounded source packs");
            for ngram in [false, true] {
                let kind = if ngram {
                    SidecarIndexKind::Ngram { n: 2 }
                } else {
                    SidecarIndexKind::Token
                };
                for legacy in [false, true] {
                    let qzi = if legacy {
                        legacy_sidecar_for_testing(&qzt, kind)
                    } else {
                        build_search_sidecar(&qzt, kind)
                    }
                    .expect("bounded sidecar builds");
                    fixtures.push(Fixture {
                        source,
                        qzt: qzt.clone(),
                        qzi,
                        ngram,
                        legacy,
                    });
                }
            }
        }
        fixtures
    })
}

#[derive(Debug)]
pub struct QziTrace {
    pub memory_open: Result<(), QztError>,
    pub file_open: Result<(), QztError>,
    pub memory_search: Option<Result<SearchReport, QztError>>,
    pub file_search: Option<Result<SearchReport, QztError>>,
}

fn query(ngram: bool, selector: u8) -> &'static str {
    const TOKEN: [&str; 5] = ["alpha", "alpha beta", "alpha alpha", "beta", "gamma"];
    const NGRAM: [&str; 5] = ["aa", "alpha", "aaaa", "αa", "beta"];
    let items = if ngram { &NGRAM } else { &TOKEN };
    items[usize::from(selector) % items.len()]
}

fn search_options(selector: u8, magnitude: u8) -> SearchOptions {
    let mut options = SearchOptions {
        max_query_bytes: 64,
        max_query_terms: 16,
        max_posting_bytes_per_query: 4096,
        max_posting_ids_per_query: 128,
        max_posting_work: 512,
        max_candidate_granules: 32,
        max_decoded_bytes: 256,
        max_physical_decoded_bytes: 256,
        max_physical_decoded_chunks: 32,
        max_search_results: 32,
    };
    // Values 8–10 straddle the alpha/beta regression fixture's boundaries.
    let exact: u64 = [0, 10, 2, 2, 2, 3, 1, 17, 17, 2, 3][usize::from(selector % 11)];
    let cap = match magnitude % 11 {
        8 => exact.saturating_sub(1),
        9 => exact,
        10 => exact + 1,
        other => [0, 1, 2, 4, 8, 16, 32, 64][usize::from(other)],
    };
    match selector % 11 {
        0 => {}
        1 => options.max_query_bytes = cap,
        2 => options.max_query_terms = cap,
        3 => options.max_posting_bytes_per_query = cap,
        4 => options.max_posting_ids_per_query = cap,
        5 => options.max_posting_work = cap,
        6 => options.max_candidate_granules = cap,
        7 => options.max_decoded_bytes = cap,
        8 => options.max_physical_decoded_bytes = cap,
        9 => options.max_physical_decoded_chunks = cap,
        _ => options.max_search_results = cap,
    }
    options
}

fn mutate_qzi(qzi: &mut [u8], legacy: bool, mode: u8, payload: &[u8]) {
    let field = match mode % 8 {
        0 => return,
        1 => (0, if legacy { 8 + 32 } else { 8 + 16 }, true, u64::MAX),
        2 => (0, if legacy { 8 + 32 } else { 8 + 16 }, true, 0),
        3 => (0, if legacy { 8 + 24 } else { 8 + 12 }, true, 2),
        4 => (0, if legacy { 8 + 8 } else { 8 }, false, u64::MAX),
        5 => (0, if legacy { 8 + 16 } else { 8 + 8 }, true, 0),
        6 => (2, 0, true, 0),
        _ => (1, 8, true, 0),
    };
    let (section, offset, short, value) = field;
    let mut replacement = if section == 0 {
        if legacy || !short {
            value.to_le_bytes().to_vec()
        } else {
            (value as u32).to_le_bytes().to_vec()
        }
    } else {
        vec![payload.first().copied().unwrap_or(0xff)]
    };
    // One caller-controlled byte reaches checksum-valid structure and posting
    // decoding, while the fixed-width mutation cannot enlarge the sidecar.
    if section == 0 && mode % 8 == 1 && legacy {
        replacement = u64::MAX.to_le_bytes().to_vec();
    }
    mutate_sidecar_section_for_testing(qzi, section, offset, &replacement)
        .expect("fixed-width patch is within the generated sidecar");
}

fn validate_hits(report: &SearchReport, source: &[u8], query: &str, ngram: bool) {
    for hit in &report.hits {
        assert_eq!(hit.source, "verified_original_bytes");
        assert!(hit.byte_length > 0 && hit.chunk_start < hit.chunk_end);
        let start = usize::try_from(hit.logical_offset).expect("small source");
        let end = start
            .checked_add(usize::try_from(hit.byte_length).expect("small hit"))
            .expect("hit end");
        let matched = source.get(start..end).expect("hit lies within source");
        if ngram {
            assert_eq!(matched, query.as_bytes());
        } else {
            assert!(
                query
                    .split_ascii_whitespace()
                    .any(|term| matched.eq_ignore_ascii_case(term.as_bytes()))
            );
        }
    }
}

/// Exercise raw or checksum-valid structured QZI on both reader paths.
pub fn exercise_qzi(data: &[u8]) -> QziTrace {
    let data = &data[..data.len().min(256)];
    let flags = data.first().copied().unwrap_or(0);
    let mut source_index = usize::from(data.get(1).copied().unwrap_or(0)) % SOURCES.len();
    if source_index == 4 && data.get(4).copied().unwrap_or(0) % 8 != 0 {
        source_index = 0; // Mutations require an actual first granule.
    }
    let ngram = flags & 1 != 0;
    let legacy = flags & 2 != 0;
    let fixture = &fixtures()[source_index * 4 + usize::from(ngram) * 2 + usize::from(legacy)];
    debug_assert_eq!((fixture.ngram, fixture.legacy), (ngram, legacy));
    let sidecar = if flags & 0x80 != 0 {
        data.get(6..).unwrap_or(&[]).to_vec()
    } else {
        let mut sidecar = fixture.qzi.clone();
        mutate_qzi(
            &mut sidecar,
            legacy,
            data.get(4).copied().unwrap_or(0),
            data.get(6..).unwrap_or(&[]),
        );
        sidecar
    };
    let query = query(ngram, data.get(2).copied().unwrap_or(0));
    let search_options = search_options(
        data.get(3).copied().unwrap_or(0),
        data.get(5).copied().unwrap_or(0),
    );
    let sidecar_limits = SidecarLimits {
        max_manifest_size: 4096,
        max_terms_size: 8192,
        max_term_count: 128,
        max_granule_count: 32,
        max_postings_size: 8192,
        max_posting_list_size: 4096,
        max_posting_bytes_per_query: 4096,
        max_decoded_posting_ids: 128,
    };
    let memory_reader = QztReader::open(&fixture.qzt).expect("generated QZT opens");
    let file_reader = QztFileReader::open_read_at(fixture.qzt.as_slice(), fixture.qzt.len() as u64)
        .expect("generated QZT file reader opens");
    let memory = QziSidecar::open_with_limits(&fixture.qzt, &sidecar, sidecar_limits);
    let file = QziFileSidecar::open_read_at_with_limits(
        sidecar.as_slice(),
        sidecar.len() as u64,
        &file_reader,
        sidecar_limits,
    );
    let memory_open = memory.as_ref().map(|_| ()).map_err(|error| *error);
    let file_open = file.as_ref().map(|_| ()).map_err(|error| *error);
    let memory_search = memory
        .map(|index| index.search(&memory_reader, query, search_options))
        .ok();
    let file_search = file
        .map(|index| index.search(&file_reader, query, search_options))
        .ok();
    for report in [&memory_search, &file_search]
        .into_iter()
        .flatten()
        .flatten()
    {
        validate_hits(report, fixture.source, query, ngram);
    }
    if flags & 0x80 == 0
        && data.get(4).copied().unwrap_or(0) % 8 == 0
        && data.get(3).copied().unwrap_or(0) % 11 == 0
    {
        let memory = memory_search
            .as_ref()
            .expect("valid memory open")
            .as_ref()
            .expect("valid memory search");
        let file = file_search
            .as_ref()
            .expect("valid file open")
            .as_ref()
            .expect("valid file search");
        assert_eq!(memory.hits, file.hits);
        assert_eq!(memory.capped, file.capped);
        assert_eq!(memory.stop_reason, file.stop_reason);
        assert_eq!(memory.incomplete_reason, file.incomplete_reason);
        assert_eq!(memory.index_complete_declared, file.index_complete_declared);
        assert_eq!(memory.index_coverage_verified, file.index_coverage_verified);
    }
    QziTrace {
        memory_open,
        file_open,
        memory_search,
        file_search,
    }
}

#[derive(Debug)]
pub struct DliTrace {
    pub decoded: Result<DenseLineIndex, QztError>,
    pub allocation_budget: u64,
    pub exact_allocation: u64,
}

fn dli_fixture(index: usize) -> (&'static [u8], Vec<ChunkEntry>, DenseLineIndex) {
    let source = SOURCES[index % SOURCES.len()];
    let plan = plan_chunks(source, options().chunker).expect("small chunk plan");
    let chunks = plan
        .chunks
        .into_iter()
        .map(|chunk| ChunkEntry {
            chunk_id: chunk.chunk_id,
            physical_offset: 0,
            compressed_size: 0,
            logical_offset: chunk.logical_offset,
            uncompressed_size: chunk.uncompressed_size,
            first_line: chunk.first_line,
            line_count: chunk.line_count,
            dictionary_id: 0,
            flags: chunk.flags,
            compressed_checksum_blake3: [0; 32],
            uncompressed_checksum_blake3: [0; 32],
        })
        .collect::<Vec<_>>();
    let index = DenseLineIndex::from_original_bytes(source, &chunks).expect("valid DLI");
    (source, chunks, index)
}

/// Exercise the direct DLI decoder with small chunk metadata and independent
/// declared values. Raw mode does not repair malformed encodings.
pub fn exercise_dli(data: &[u8]) -> DliTrace {
    let data = &data[..data.len().min(256)];
    let flags = data.first().copied().unwrap_or(0);
    let source_index = usize::from(data.get(1).copied().unwrap_or(0)) % SOURCES.len();
    let (source, mut chunks, valid) = dli_fixture(source_index);
    let exact = chunks.len() as u64 * std::mem::size_of::<DenseLineEntry>() as u64
        + chunks.iter().map(|chunk| chunk.line_count * 8).sum::<u64>();
    let budget = match data.get(3).copied().unwrap_or(3) % 5 {
        0 => 0,
        1 => exact.saturating_sub(1),
        2 => exact,
        3 => exact + 1,
        _ => 256,
    };
    if let Some(first) = chunks.first_mut() {
        if flags & 0x40 != 0 {
            first.line_count = u64::MAX;
        }
        if flags & 0x20 != 0 {
            first.uncompressed_size = u64::MAX;
        }
    }
    let mut bytes = if flags & 0x80 != 0 {
        data.get(4..).unwrap_or(&[]).to_vec()
    } else {
        valid.encode().expect("valid DLI encodes")
    };
    if flags & 0x80 == 0 {
        match data.get(2).copied().unwrap_or(0) % 8 {
            0 => {}
            1 => {
                bytes.splice(
                    0..1,
                    [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1],
                );
            }
            2 => {
                if bytes.len() >= 3 {
                    bytes[2] = 0x7f;
                }
            }
            3 => {
                bytes.pop();
            }
            4 => {
                if !bytes.is_empty() {
                    bytes.splice(0..1, [0x80, 0]);
                }
            }
            5 => {
                if let Some(last) = bytes.last_mut() {
                    *last = 0;
                }
            }
            6 => bytes.push(0),
            _ => {
                if let Some(last) = bytes.last_mut() {
                    *last = data.get(4).copied().unwrap_or(0xff);
                }
            }
        }
    }
    let decoded = DenseLineIndex::decode_for_chunks_with_limit(&bytes, &chunks, budget);
    if flags & 0xe0 == 0 && data.get(2).copied().unwrap_or(0) % 8 == 0 && budget >= exact {
        let index = decoded
            .as_ref()
            .expect("valid DLI must decode within budget");
        assert_eq!(index, &valid);
        for (position, chunk) in chunks.iter().enumerate() {
            let start = chunk.logical_offset as usize;
            let end = start + chunk.uncompressed_size as usize;
            index
                .verify_chunk(position, &source[start..end], chunk.flags)
                .expect("valid DLI matches original bytes");
        }
    }
    DliTrace {
        decoded,
        allocation_budget: budget,
        exact_allocation: exact,
    }
}

fn assert_budget_seed(name: &str, trace: &QziTrace) {
    let (kind, point) = name
        .strip_prefix("budget_")
        .and_then(|rest| rest.rsplit_once('_'))
        .expect("budget seed name");
    let early_error = matches!(
        kind,
        "query_bytes" | "query_terms" | "posting_bytes" | "posting_ids" | "posting_work"
    ) && matches!(point, "zero" | "below");
    let limited = matches!(point, "zero" | "below")
        || (kind == "spans" && matches!(point, "one" | "two" | "exact"));
    let stop = match kind {
        "candidates" => Some("max_candidate_granules"),
        "logical_bytes" => Some("max_decoded_bytes"),
        "physical_bytes" => Some("max_physical_decoded_bytes"),
        "physical_chunks" => Some("max_physical_decoded_chunks"),
        "spans" => Some("max_search_results"),
        _ => None,
    };
    for result in [&trace.memory_search, &trace.file_search] {
        let result = result.as_ref().expect("budget search reached");
        if early_error {
            assert_eq!(
                result.as_ref().map(|_| ()).map_err(|error| *error),
                Err(QztError::ResourceLimitExceeded),
                "{name}: wrong budget rejection"
            );
            continue;
        }
        let report = result
            .as_ref()
            .unwrap_or_else(|error| panic!("{name}: unexpected {error:?}"));
        assert_eq!(report.stop_reason, stop.filter(|_| limited), "{name}: stop");
        if kind == "spans" {
            let expected_hits = match point {
                "zero" => 0,
                "one" => 1,
                "two" | "below" => 2,
                _ => 3,
            };
            assert_eq!(report.hits.len(), expected_hits, "{name}: result count");
        }
        if limited && matches!(kind, "logical_bytes" | "physical_bytes" | "physical_chunks") {
            assert_eq!(
                report.metrics.physical_decoded_bytes, 0,
                "{name}: decoded too early"
            );
        }
    }
    if kind == "candidates" && limited {
        let file = trace.file_search.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(
            file.metrics.candidate_chunks, 0,
            "{name}: lazy granule was fetched"
        );
    }
}

/// Replay every tracked initial seed through the same entry points used by libFuzzer.
pub fn replay_checked_in_seeds() {
    for entry in std::fs::read_dir("seeds/qzi_search").expect("tracked QZI seed directory") {
        let entry = entry.expect("QZI seed entry");
        let bytes = std::fs::read(entry.path()).expect("seed bytes");
        let trace = exercise_qzi(&bytes);
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("valid_")
            || name.starts_with("budget_")
            || name == "ngram_overlap"
            || name == "token_and_boundary"
        {
            assert!(
                trace.memory_open.is_ok() && trace.file_open.is_ok(),
                "{name}: valid sidecar did not open"
            );
            assert!(
                trace.memory_search.is_some() && trace.file_search.is_some(),
                "{name}: search not reached"
            );
            if name.starts_with("budget_") {
                assert_budget_seed(&name, &trace);
            }
            if name == "token_and_boundary" || name == "ngram_overlap" {
                let expected = if name == "token_and_boundary" {
                    vec![0, 6, 12]
                } else {
                    vec![0, 1, 2]
                };
                for result in [&trace.memory_search, &trace.file_search] {
                    let hits = &result.as_ref().unwrap().as_ref().unwrap().hits;
                    assert_eq!(
                        hits.iter()
                            .map(|hit| hit.logical_offset)
                            .collect::<Vec<_>>(),
                        expected,
                        "{name}: matcher span positions"
                    );
                }
            }
        } else if name.starts_with("huge_span_")
            || name.starts_with("empty_span_")
            || name.starts_with("outside_span_")
            || name.starts_with("overflow_offset_")
            || name.starts_with("zero_length_")
        {
            assert!(
                trace.memory_open.is_err(),
                "{name}: malformed granule accepted in memory"
            );
            assert!(
                trace.file_open.is_ok(),
                "{name}: lazy file open should reach search"
            );
            assert!(
                trace.file_search.as_ref().is_some_and(Result::is_err),
                "{name}: malformed granule not rejected during file search"
            );
        } else if name.starts_with("posting_mutation_") || name.starts_with("term_mutation_") {
            assert!(
                trace.memory_open.is_err()
                    || trace.file_open.is_err()
                    || trace.memory_search.as_ref().is_some_and(Result::is_err)
                    || trace.file_search.as_ref().is_some_and(Result::is_err),
                "{name}: mutation did not exercise decoder"
            );
        } else if name == "raw_bad_magic" {
            assert!(trace.memory_open.is_err() && trace.file_open.is_err());
        } else {
            panic!("unclassified QZI seed: {name}");
        }
    }
    for entry in std::fs::read_dir("seeds/dli_decode").expect("tracked DLI seeds") {
        let entry = entry.expect("DLI seed entry");
        let bytes = std::fs::read(entry.path()).expect("DLI seed bytes");
        let trace = exercise_dli(&bytes);
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("valid_")
            || name == "budget_exact"
            || name == "budget_above"
            || name == "budget_wide"
        {
            assert!(trace.decoded.is_ok(), "{name}: valid DLI did not decode");
        } else {
            assert!(
                trace.decoded.is_err(),
                "{name}: malformed or under-budget DLI decoded"
            );
        }
        if name == "budget_below" || name == "budget_zero" {
            assert!(
                trace.exact_allocation > std::mem::size_of::<DenseLineEntry>() as u64 + 8,
                "{name}: must span multiple chunk allocations"
            );
            assert_eq!(
                trace.decoded,
                Err(QztError::ResourceLimitExceeded),
                "{name}: allocation boundary"
            );
        } else if !name.starts_with("valid_")
            && !matches!(
                name.as_str(),
                "budget_exact" | "budget_above" | "budget_wide"
            )
        {
            let expected = if matches!(
                name.as_str(),
                "truncated" | "raw_truncated_varint" | "offset_mutation"
            ) {
                QztError::UnexpectedEof
            } else {
                QztError::ChunkTableInvalid
            };
            assert_eq!(
                trace.decoded,
                Err(expected),
                "{name}: wrong decoder rejection"
            );
        }
        if name == "valid_continuation" {
            assert!(
                trace
                    .decoded
                    .unwrap()
                    .entries
                    .iter()
                    .any(|entry| entry.line_start_offsets.is_empty())
            );
        }
    }
}
