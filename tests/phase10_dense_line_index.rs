use std::time::Instant;

use qzt::chunk_table::ChunkEntry;
use qzt::dense_line_index::{DenseLineEntry, DenseLineIndex};
use qzt::error::QztError;
use qzt::fixed::{FooterTrailer, Header};
use qzt::format::{FOOTER_TRAILER_LEN, HEADER_LEN};
use qzt::limits::ResourceLimits;
use qzt::reader::{QztFileReader, QztReader, VerifyLevel};
use qzt::schema::{
    BlockDescriptor, BlockRef, Checksum, DocumentEntry, DocumentIndex, FooterPayload, IndexRoot,
};
use qzt::skeleton::open_skeleton_details;
use qzt::writer::{WriterBuilder, pack_bytes_with_dense_line_index_override};
mod support;

fn pack_dense_fixture(
    input: &[u8],
    container_id: [u8; 16],
    options: qzt::WriterOptions,
) -> qzt::Result<Vec<u8>> {
    WriterBuilder::new()
        .container_id(container_id)
        .options(options)
        .dense_line_index(true)
        .pack(input)
}

fn pack_memory_fixture(
    input: &[u8],
    container_id: [u8; 16],
    options: qzt::WriterOptions,
    document_index: DocumentIndex,
) -> qzt::Result<Vec<u8>> {
    WriterBuilder::new()
        .container_id(container_id)
        .options(options)
        .profile("memory")
        .document_index(document_index)
        .pack(input)
}

fn newline_corpus(line_count: u64) -> Vec<u8> {
    let mut input = Vec::new();
    for index in 0..line_count {
        input.extend_from_slice(format!("line-{index:04}\n").as_bytes());
    }
    input
}

fn single_document_memory_profile_index(
    input: &[u8],
    container_id: [u8; 16],
    line_count: u64,
) -> DocumentIndex {
    DocumentIndex {
        container_id,
        documents: vec![DocumentEntry::new(
            "all",
            0,
            input.len() as u64,
            0,
            line_count,
            0,
            1,
            Checksum::blake3(input),
        )],
    }
}

#[test]
fn dense_line_index_reads_final_line_without_newline() {
    let input = b"alpha\nbeta\ngamma";
    let container = pack_dense_fixture(input, [0xa0; 16], support::writer_options(8, 8))
        .expect("dense container should pack");
    let reader = QztReader::open(container).expect("dense container should open");

    assert_eq!(reader.read_line_raw(2), Ok(b"gamma".to_vec()));
    assert_eq!(
        reader
            .verify(VerifyLevel::Deep)
            .expect("deep verify should pass")
            .decoded_bytes,
        input.len() as u64
    );
}

#[test]
fn dense_line_index_count_mismatch_is_rejected() {
    let input = b"alpha\nbeta\n";
    let mut container = pack_dense_fixture(input, [0xa1; 16], support::writer_options(32, 32))
        .expect("dense container should pack");
    let details = open_skeleton_details(&container).expect("container should open");
    let dense = details
        .dense_line_index
        .expect("dense line index should be present");
    let mut entries = dense.entries;
    entries[0].line_start_offsets.pop();
    container = pack_bytes_with_dense_line_index_override(
        input,
        [0xa1; 16],
        support::writer_options(32, 32),
        DenseLineIndex { entries },
    )
    .expect("corrupt dense container should pack structurally");

    assert_eq!(
        QztReader::open(container).map(|_| ()),
        Err(QztError::ChunkTableInvalid)
    );
}

#[test]
fn dense_decoder_rejects_impossible_declared_offsets_before_reserve() {
    let entry = ChunkEntry {
        chunk_id: 0,
        physical_offset: 0,
        compressed_size: 1,
        logical_offset: 0,
        uncompressed_size: 1_u64 << 61,
        first_line: 0,
        line_count: 1_u64 << 61,
        dictionary_id: 0,
        flags: 0,
        compressed_checksum_blake3: [0; 32],
        uncompressed_checksum_blake3: [0; 32],
    };
    // One entry, chunk zero, and 2^61 offsets, with no offset bytes.
    let bytes = [1, 0, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x20];
    assert_eq!(
        DenseLineIndex::decode_for_chunks(&bytes, &[entry]),
        Err(QztError::UnexpectedEof)
    );
}

fn assert_both_reject(container: &[u8], limits: ResourceLimits, expected: QztError) {
    assert_eq!(
        QztReader::open_with_limits(container, limits).map(|_| ()),
        Err(expected)
    );
    assert_eq!(
        QztFileReader::open_read_at_with_limits(container, container.len() as u64, limits)
            .map(|_| ()),
        Err(expected)
    );
}

#[test]
fn dense_decoder_checks_counts_varints_and_ranges_without_allocating() {
    let entry = ChunkEntry {
        chunk_id: 0,
        physical_offset: 0,
        compressed_size: 1,
        logical_offset: 0,
        uncompressed_size: 2,
        first_line: 0,
        line_count: 2,
        dictionary_id: 0,
        flags: 0,
        compressed_checksum_blake3: [0; 32],
        uncompressed_checksum_blake3: [0; 32],
    };
    let cases: &[(&[u8], QztError)] = &[
        (&[0], QztError::ChunkTableInvalid),                // entry count
        (&[1, 1, 2, 0, 1], QztError::ChunkTableInvalid),    // ID
        (&[1, 0, 1, 0], QztError::ChunkTableInvalid),       // offset count
        (&[1, 0, 2, 0], QztError::UnexpectedEof),           // count exceeds bytes
        (&[1, 0, 2, 0, 0], QztError::ChunkTableInvalid),    // zero delta
        (&[1, 0, 2, 0, 2], QztError::ChunkTableInvalid),    // range
        (&[1, 0, 2, 0, 1, 0], QztError::ChunkTableInvalid), // trailing data
        (&[0x81, 0, 0, 2, 0, 1], QztError::ChunkTableInvalid), // noncanonical
        (&[1, 0, 2, 0x80], QztError::UnexpectedEof),        // truncated varint
    ];
    for (bytes, expected) in cases {
        assert_eq!(
            DenseLineIndex::decode_for_chunks(bytes, std::slice::from_ref(&entry)),
            Err(*expected),
            "{bytes:?}"
        );
    }

    let mut wide_entry = entry.clone();
    wide_entry.uncompressed_size = u64::MAX;
    // First offset is u64::MAX - 1, then a delta of 2 overflows u64.
    let overflow = [
        1, 0, 2, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1, 2,
    ];
    assert_eq!(
        DenseLineIndex::decode_for_chunks(&overflow, &[wide_entry]),
        Err(QztError::ChunkTableInvalid)
    );

    let mut impossible = entry.clone();
    impossible.line_count = 3;
    assert_eq!(
        DenseLineIndex::decode_for_chunks(&[1, 0, 3, 0, 1, 1], &[impossible]),
        Err(QztError::ChunkTableInvalid)
    );
    let mut continuation = entry;
    continuation.line_count = 0;
    continuation.flags = qzt::chunk_table::STARTS_WITH_LINE_CONTINUATION;
    assert_eq!(
        DenseLineIndex::decode_for_chunks(&[1, 0, 0], &[continuation]),
        Ok(DenseLineIndex {
            entries: vec![DenseLineEntry {
                chunk_id: 0,
                line_start_offsets: vec![]
            }]
        })
    );
}

#[test]
fn dense_reader_preserves_newline_and_empty_inputs() {
    for input in [b"".as_slice(), b"a", b"a\n", b"a\r\nb\r\n", b"a\nb"] {
        let container =
            pack_dense_fixture(input, [0xc2; 16], support::writer_options(3, 3)).expect("pack");
        assert_eq!(
            QztReader::open(&container)
                .expect("memory open")
                .export_all(),
            Ok(input.to_vec())
        );
        assert_eq!(
            QztFileReader::open_read_at(&container[..], container.len() as u64)
                .expect("file open")
                .export_all(),
            Ok(input.to_vec())
        );
    }
}

#[test]
fn dense_allocation_budget_is_cumulative_and_shared_by_readers() {
    let input = b"a\nb\n";
    let container =
        pack_dense_fixture(input, [0xc0; 16], support::writer_options(2, 2)).expect("pack");
    let details = open_skeleton_details(&container).expect("open");
    assert!(details.chunk_entries.len() >= 2);
    let entry_bytes = details.chunk_entries.len() * std::mem::size_of::<DenseLineEntry>();
    let offset_bytes = usize::try_from(details.summary.line_count).expect("small fixture")
        * std::mem::size_of::<u64>();
    let budget = (entry_bytes + offset_bytes) as u64;
    assert!(budget - 1 > std::mem::size_of::<DenseLineEntry>() as u64 + 8);
    let limits = ResourceLimits {
        max_dense_line_index_allocation: budget - 1,
        ..ResourceLimits::default()
    };
    assert_both_reject(&container, limits, QztError::ResourceLimitExceeded);
    let limits = ResourceLimits {
        max_dense_line_index_allocation: budget,
        ..ResourceLimits::default()
    };
    assert!(QztReader::open_with_limits(&container, limits).is_ok());
    assert!(
        QztFileReader::open_read_at_with_limits(&container[..], container.len() as u64, limits)
            .is_ok()
    );
}

#[test]
fn impossible_declared_lines_reach_chunk_table_before_dli_allocation() {
    let container = rebuilt_container(1_u64 << 61, Some(&[1, 0, 0]));
    assert_both_reject(
        &container,
        ResourceLimits::default(),
        QztError::ChunkTableInvalid,
    );
    let no_dli = rebuilt_container(u64::MAX, None);
    assert_both_reject(
        &no_dli,
        ResourceLimits::default(),
        QztError::ChunkTableInvalid,
    );
}

#[test]
fn checksum_valid_malformed_dli_reaches_decoder_in_both_readers() {
    let truncated = rebuilt_container(1, Some(&[1, 0, 1]));
    assert_both_reject(
        &truncated,
        ResourceLimits::default(),
        QztError::UnexpectedEof,
    );
    let out_of_range = rebuilt_container(1, Some(&[1, 0, 1, 1]));
    assert_both_reject(
        &out_of_range,
        ResourceLimits::default(),
        QztError::ChunkTableInvalid,
    );
}

fn rebuilt_container(line_count: u64, dli: Option<&[u8]>) -> Vec<u8> {
    let input = b"x";
    let original =
        pack_dense_fixture(input, [0xc1; 16], support::writer_options(16, 16)).expect("pack");
    let details = open_skeleton_details(&original).expect("open original");
    let mut header = Header::decode(&original[..HEADER_LEN]).expect("header");
    let mut output =
        original[..usize::try_from(header.metadata_offset).expect("small fixture")].to_vec();
    let mut metadata = details.metadata;
    metadata.line_count = line_count;
    metadata.dense_line_index = dli.is_some();
    let metadata_bytes = metadata.encode().expect("metadata");
    header.metadata_size = metadata_bytes.len() as u64;
    output.extend_from_slice(&metadata_bytes);
    let metadata_ref = BlockRef {
        offset: header.metadata_offset,
        size: header.metadata_size,
        checksum: Checksum::blake3(&metadata_bytes),
    };

    let mut blocks = Vec::new();
    if let Some(dli) = dli {
        blocks.push(BlockDescriptor::dense_line_index(
            output.len() as u64,
            dli.len() as u64,
            Checksum::blake3(dli),
        ));
        output.extend_from_slice(dli);
    }
    let mut entry = details.chunk_entries[0].clone();
    entry.line_count = line_count;
    let table = entry.encode();
    blocks.insert(
        0,
        BlockDescriptor::chunk_table(
            output.len() as u64,
            table.len() as u64,
            Checksum::blake3(&table),
        ),
    );
    output.extend_from_slice(&table);

    header.index_hint_offset = output.len() as u64;
    let root = IndexRoot {
        container_id: header.container_id,
        blocks,
        original_size: input.len() as u64,
        original_checksum: Checksum::blake3(input),
        chunk_count: 1,
        line_count,
    };
    let root_bytes = root.encode().expect("root");
    let root_ref = BlockRef {
        offset: header.index_hint_offset,
        size: root_bytes.len() as u64,
        checksum: Checksum::blake3(&root_bytes),
    };
    output.extend_from_slice(&root_bytes);

    let footer_offset = output.len() as u64;
    let mut final_size = 0;
    let footer = loop {
        let candidate = FooterPayload {
            container_id: header.container_id,
            index_root: root_ref.clone(),
            metadata: metadata_ref.clone(),
            final_file_size: final_size,
            footer_flags: 0,
            container_checksum: None,
        };
        let bytes = candidate.encode().expect("footer");
        let next = footer_offset + bytes.len() as u64 + FOOTER_TRAILER_LEN as u64;
        if next == final_size {
            break candidate;
        }
        final_size = next;
    };
    let footer_bytes = footer.encode().expect("footer");
    let trailer = FooterTrailer {
        footer_payload_offset: footer_offset,
        footer_payload_size: footer_bytes.len() as u64,
        footer_payload_checksum_blake3: Checksum::blake3(&footer_bytes).value,
    };
    output[..HEADER_LEN].copy_from_slice(&header.encode());
    output.extend_from_slice(&footer_bytes);
    output.extend_from_slice(&trailer.encode());
    output
}

#[test]
fn deep_verify_detects_dense_line_index_disagreement() {
    let input = b"alpha\nbeta\ngamma\n";
    let dense = DenseLineIndex {
        entries: vec![DenseLineEntry {
            chunk_id: 0,
            line_start_offsets: vec![0, 7, 12],
        }],
    };
    let container = pack_bytes_with_dense_line_index_override(
        input,
        [0xa2; 16],
        support::writer_options(64, 64),
        dense,
    )
    .expect("stale dense container should pack structurally");
    let reader = QztReader::open(container).expect("stale dense count should open");

    assert_eq!(
        reader.verify(VerifyLevel::Deep),
        Err(QztError::ChunkTableInvalid)
    );
}

#[test]
fn memory_profile_below_dense_threshold_omits_dense_line_index() {
    let line_count = 1024;
    let container_id = [0xb5; 16];
    let input = newline_corpus(line_count);
    let document_index = single_document_memory_profile_index(&input, container_id, line_count);
    let options = support::writer_options(65_536, 65_536);

    let container = pack_memory_fixture(&input, container_id, options, document_index)
        .expect("memory profile should pack");
    let details = open_skeleton_details(&container).expect("memory profile should open");

    assert_eq!(details.metadata.profile, "memory");
    assert!(!details.metadata.dense_line_index);
    assert!(details.dense_line_index.is_none());
    assert!(details.metadata.document_index);
    assert!(details.document_index.is_some());

    let reader = QztReader::open(container).expect("memory profile reader should open");
    assert_eq!(reader.read_line_raw(0), Ok(b"line-0000\n".to_vec()));
    assert_eq!(
        reader.read_line_raw(line_count - 1),
        Ok(format!("line-{:04}\n", line_count - 1).into_bytes())
    );
    assert!(reader.verify(VerifyLevel::Deep).is_ok());
}

#[test]
fn memory_profile_at_dense_threshold_writes_dense_line_index() {
    let line_count = 2048;
    let container_id = [0xb6; 16];
    let input = newline_corpus(line_count);
    let document_index = single_document_memory_profile_index(&input, container_id, line_count);
    let options = support::writer_options(65_536, 65_536);

    let container = pack_memory_fixture(&input, container_id, options, document_index)
        .expect("memory profile should pack");
    let details = open_skeleton_details(&container).expect("memory profile should open");

    assert_eq!(details.metadata.profile, "memory");
    assert!(details.metadata.dense_line_index);
    assert!(details.dense_line_index.is_some());
    assert!(details.metadata.document_index);
    assert!(details.document_index.is_some());

    let reader = QztReader::open(container).expect("memory profile reader should open");
    assert_eq!(reader.read_line_raw(0), Ok(b"line-0000\n".to_vec()));
    assert_eq!(
        reader.read_line_raw(line_count - 1),
        Ok(format!("line-{:04}\n", line_count - 1).into_bytes())
    );
    assert!(reader.verify(VerifyLevel::Deep).is_ok());
}

#[test]
fn sparse_vs_dense_line_lookup_benchmark_records_threshold_evidence() {
    let line_count = 2048;
    let mut input = Vec::new();
    for index in 0..line_count {
        input.extend_from_slice(format!("line-{index:04}\n").as_bytes());
    }

    let sparse_container = qzt::writer::pack_bytes_with_container_id(
        &input,
        [0xa3; 16],
        support::writer_options(128, 128),
    )
    .expect("sparse container should pack");
    let dense_container = pack_dense_fixture(&input, [0xa4; 16], support::writer_options(128, 128))
        .expect("dense container should pack");
    let sparse_reader = QztReader::open(sparse_container).expect("sparse should open");
    let dense_reader = QztReader::open(dense_container).expect("dense should open");

    let started = Instant::now();
    for line in (0..line_count).step_by(7) {
        let _ = sparse_reader
            .read_line_raw(line)
            .expect("sparse line should read");
    }
    let sparse_elapsed = started.elapsed();

    let started = Instant::now();
    for line in (0..line_count).step_by(7) {
        let _ = dense_reader
            .read_line_raw(line)
            .expect("dense line should read");
    }
    let dense_elapsed = started.elapsed();

    assert!(sparse_elapsed.as_nanos() > 0);
    assert!(dense_elapsed.as_nanos() > 0);
    eprintln!(
        "phase10_dense_bench lines={} sparse_us={:.3} dense_us={:.3} threshold_decision=enable_dense_for_memory_profile_at_or_above_2048_lines",
        line_count,
        sparse_elapsed.as_secs_f64() * 1_000_000.0,
        dense_elapsed.as_secs_f64() * 1_000_000.0
    );
}
