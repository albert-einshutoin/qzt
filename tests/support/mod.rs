#![allow(dead_code)]

use std::io;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use tempfile::TempDir;

use qzt::{
    Checksum, ChunkerOptions, DocumentEntry, ReadAt, SearchReport, WriterOptions,
    pack_bytes_with_container_id,
};

/// Return a process-private temporary root for integration-test paths.
///
/// Many tests need stable child paths for CLI assertions. A securely created
/// parent keeps those predictable child names inaccessible to other users and
/// processes, while `TempDir` removes the complete tree when the test binary
/// exits.
pub fn secure_temp_root() -> &'static Path {
    static ROOT: OnceLock<TempDir> = OnceLock::new();

    ROOT.get_or_init(|| {
        tempfile::Builder::new()
            .prefix("qzt-integration-test-")
            .tempdir()
            .expect("secure integration-test temporary root should be created")
    })
    .path()
}

/// Compare semantically-equivalent search behavior between two execution paths.
///
/// Intentionally excluded fields:
/// - `metrics.query_time_ms`: runtime dependent and non-deterministic
/// - `metrics.index_size_bytes`: defined differently by source of metric
///   (`Raw` sidecar reports estimated in-memory bytes, while file sidecar
///   reports serialized section payload bytes; on skip-heavy indexes this can
///   make file-sidecar bytes smaller than in-memory estimates)
/// - `metrics.posting_bytes_read`: differs when skip-data is simulated
/// - `planner.used_skip_data`: file sidecar keeps it false even when the
///   in-memory index would use skip data
/// - `metrics.candidate_chunks`:
///   when capped=false, in-memory and file sidecar both report counts
///   consistently; when capped=true, file sidecar returns 0 early before counting
///   candidate chunks (see sidecar.rs), so this field is compared only when uncapped.
pub fn assert_semantic_report_eq(left: &SearchReport, right: &SearchReport, label: &str) {
    assert_eq!(left.hits, right.hits, "hits mismatch: {label}");
    assert_eq!(left.capped, right.capped, "capped mismatch: {label}");
    assert_eq!(
        left.metrics.term_lookups, right.metrics.term_lookups,
        "term_lookups mismatch: {label}"
    );
    assert_eq!(
        left.metrics.verified_matches, right.metrics.verified_matches,
        "verified_matches mismatch: {label}"
    );
    assert_eq!(
        left.metrics.candidate_granules, right.metrics.candidate_granules,
        "candidate_granules mismatch: {label}"
    );
    assert_eq!(
        left.metrics.decoded_bytes, right.metrics.decoded_bytes,
        "decoded_bytes mismatch: {label}"
    );
    assert_eq!(
        left.metrics.physical_decoded_bytes, right.metrics.physical_decoded_bytes,
        "physical_decoded_bytes mismatch: {label}"
    );
    if !left.capped {
        assert_eq!(
            left.metrics.candidate_chunks, right.metrics.candidate_chunks,
            "candidate_chunks mismatch: {label}"
        );
    }
    assert_eq!(
        left.incomplete_reason, right.incomplete_reason,
        "incomplete_reason mismatch: {label}"
    );
    assert_eq!(
        left.planner.selected_keys, right.planner.selected_keys,
        "planner.selected_keys mismatch: {label}"
    );
    assert_eq!(
        left.planner.missing_keys, right.planner.missing_keys,
        "planner.missing_keys mismatch: {label}"
    );
    assert_eq!(
        left.planner.high_df_keys, right.planner.high_df_keys,
        "planner.high_df_keys mismatch: {label}"
    );
}

pub fn chunker_options(target_chunk_size: usize, max_chunk_size: usize) -> ChunkerOptions {
    ChunkerOptions {
        target_chunk_size,
        max_chunk_size,
    }
}

pub fn writer_options(target_chunk_size: usize, max_chunk_size: usize) -> WriterOptions {
    WriterOptions {
        chunker: chunker_options(target_chunk_size, max_chunk_size),
        zstd_level: 0,
    }
}

pub fn small_chunk_options() -> WriterOptions {
    writer_options(8, 32)
}

pub fn pack_with_container_id(
    input: &[u8],
    container_id: [u8; 16],
    target_chunk_size: usize,
    max_chunk_size: usize,
) -> Vec<u8> {
    pack_bytes_with_container_id(
        input,
        container_id,
        writer_options(target_chunk_size, max_chunk_size),
    )
    .expect("pack should work")
}

pub fn output_success(command: &mut Command) -> Vec<u8> {
    let output = command.output().expect("command should run");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

pub fn assert_success(command: &mut Command) {
    // Keep one command-execution contract so assertion diagnostics cannot drift
    // between callers that need stdout and callers that only need success.
    drop(output_success(command));
}

#[derive(Clone)]
pub struct CountingReadAt {
    pub bytes: Arc<Vec<u8>>,
    pub reads: Arc<Mutex<Vec<(u64, u64)>>>,
}

impl CountingReadAt {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Arc::new(bytes),
            reads: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ReadAt for CountingReadAt {
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        self.reads
            .lock()
            .map_err(|_| io::Error::other("poisoned reads lock"))?
            .push((offset, buf.len() as u64));
        let start = usize::try_from(offset)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset too large"))?;
        let end = start
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range overflow"))?;
        let source = self
            .bytes
            .get(start..end)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "short read"))?;
        buf.copy_from_slice(source);
        Ok(())
    }
}

#[derive(Clone)]
pub struct DocumentFixture<'a> {
    pub doc_id: &'a str,
    pub input: &'a [u8],
    pub logical_offset: u64,
    pub byte_length: u64,
    pub first_line: u64,
    pub line_count: u64,
    pub chunk_start: u64,
    pub chunk_end: u64,
}

pub fn document(fixture: &DocumentFixture<'_>) -> DocumentEntry {
    let start = usize::try_from(fixture.logical_offset).unwrap_or(fixture.input.len());
    let end = start
        .checked_add(usize::try_from(fixture.byte_length).unwrap_or(0))
        .unwrap_or(start)
        .min(fixture.input.len());
    let range = fixture.input.get(start..end).unwrap_or(&[]);
    DocumentEntry::new(
        fixture.doc_id,
        fixture.logical_offset,
        fixture.byte_length,
        fixture.first_line,
        fixture.line_count,
        fixture.chunk_start,
        fixture.chunk_end,
        Checksum::blake3(range),
    )
}

#[derive(Clone)]
pub struct DocumentFixtureWithChecksum<'a> {
    pub doc_id: &'a str,
    pub input: &'a [u8],
    pub logical_offset: u64,
    pub byte_length: u64,
    pub first_line: u64,
    pub line_count: u64,
    pub chunk_start: u64,
    pub chunk_end: u64,
    pub checksum_bytes: &'a [u8],
}

pub fn document_with_checksum(fixture: &DocumentFixtureWithChecksum<'_>) -> DocumentEntry {
    let fallback_end = usize::try_from(fixture.logical_offset)
        .ok()
        .and_then(|start| start.checked_add(usize::try_from(fixture.byte_length).ok()?))
        .unwrap_or(fixture.input.len());
    let range = fixture
        .input
        .get(
            usize::try_from(fixture.logical_offset).unwrap_or(0)
                ..fallback_end.min(fixture.input.len()),
        )
        .unwrap_or(&[]);

    let checksum = if fixture.checksum_bytes == b"actual" {
        Checksum::blake3(range)
    } else {
        Checksum::blake3(fixture.checksum_bytes)
    };

    DocumentEntry::new(
        fixture.doc_id,
        fixture.logical_offset,
        fixture.byte_length,
        fixture.first_line,
        fixture.line_count,
        fixture.chunk_start,
        fixture.chunk_end,
        checksum,
    )
}
