use std::io::Write;

use qzt::chunk_table::ChunkEntry;
use qzt::chunker::{ChunkerOptions, plan_chunks};
use qzt::error::QztError;
use qzt::fixed::{FooterTrailer, Header};
use qzt::format::{FOOTER_TRAILER_LEN, HEADER_LEN};
use qzt::limits::ResourceLimits;
use qzt::reader::QztReader;
use qzt::schema::{
    BlockDescriptor, BlockRef, Checksum, DictionaryBlock, DictionaryEntry, FooterPayload,
    IndexRoot, Metadata, MetadataOptions,
};

const CONTAINER_ID: [u8; 16] = [0x88; 16];
const DICTIONARY_ID: u32 = 7;

struct ExtraBlock {
    block_type: &'static str,
    required: bool,
    codec: &'static str,
    payload: Vec<u8>,
}

#[test]
fn dictionary_compressed_fixture_exports_exactly() {
    let input = b"alpha alpha alpha\nbeta beta beta\n";
    let dictionary = b"alpha beta gamma delta alpha beta gamma delta";
    let container = build_container(
        input,
        Some(dictionary_block(vec![dictionary_entry(
            DICTIONARY_ID,
            dictionary.to_vec(),
            None,
        )])),
        DICTIONARY_ID,
        &[],
    );

    let reader = QztReader::open(container).expect("dictionary container should open");

    assert_eq!(reader.export_all(), Ok(input.to_vec()));
}

#[test]
fn missing_dictionary_is_rejected_at_open() {
    let container = build_container(b"abc\n", None, DICTIONARY_ID, &[]);

    assert_eq!(
        QztReader::open(container).map(|_| ()),
        Err(QztError::MissingDictionary)
    );
}

#[test]
fn duplicate_dictionary_id_is_rejected() {
    let dictionary = b"duplicate dictionary bytes".to_vec();
    let container = build_container(
        b"abc\n",
        Some(dictionary_block(vec![
            dictionary_entry(DICTIONARY_ID, dictionary.clone(), None),
            dictionary_entry(DICTIONARY_ID, dictionary, None),
        ])),
        0,
        &[],
    );

    assert_eq!(
        QztReader::open(container).map(|_| ()),
        Err(QztError::ContainerCorrupt)
    );
}

#[test]
fn dictionary_checksum_mismatch_is_rejected() {
    let dictionary = b"checksum dictionary bytes".to_vec();
    let container = build_container(
        b"abc\n",
        Some(dictionary_block(vec![dictionary_entry(
            DICTIONARY_ID,
            dictionary,
            Some([0x11; 32]),
        )])),
        0,
        &[],
    );

    assert_eq!(
        QztReader::open(container).map(|_| ()),
        Err(QztError::DictionaryChecksumMismatch)
    );
}

#[test]
fn unknown_optional_block_is_ignored_but_unknown_required_block_is_rejected() {
    let optional = ExtraBlock {
        block_type: "future_index",
        required: false,
        codec: "future-codec",
        payload: b"ignored".to_vec(),
    };
    let optional_container = build_container(b"abc\n", None, 0, &[optional]);

    let reader = QztReader::open(optional_container).expect("optional block should be ignored");
    assert_eq!(reader.export_all(), Ok(b"abc\n".to_vec()));

    let required = ExtraBlock {
        block_type: "future_index",
        required: true,
        codec: "future-codec",
        payload: b"fatal".to_vec(),
    };
    let required_container = build_container(b"abc\n", None, 0, &[required]);

    assert_eq!(
        QztReader::open(required_container).map(|_| ()),
        Err(QztError::UnknownRequiredBlock)
    );
}

#[test]
fn resource_limits_are_enforced_before_decode() {
    let chunk_limited = build_container(b"abcd", None, 0, &[]);
    let limits = ResourceLimits {
        max_uncompressed_chunk_size: 3,
        ..ResourceLimits::default()
    };

    assert_eq!(
        QztReader::open_with_limits(chunk_limited, limits).map(|_| ()),
        Err(QztError::ResourceLimitExceeded)
    );

    let dictionary = b"dictionary limit bytes".to_vec();
    let dictionary_limited = build_container(
        b"abcd",
        Some(dictionary_block(vec![dictionary_entry(
            DICTIONARY_ID,
            dictionary,
            None,
        )])),
        0,
        &[],
    );
    let limits = ResourceLimits {
        max_dictionary_size: 4,
        ..ResourceLimits::default()
    };

    assert_eq!(
        QztReader::open_with_limits(dictionary_limited, limits).map(|_| ()),
        Err(QztError::ResourceLimitExceeded)
    );
}

#[test]
fn known_extension_type_as_required_is_rejected_by_v01_core() {
    // "token_index" was previously in is_known_block_type but is not processed by the
    // v0.1 Core reader. A required=true token_index block signals a capability the
    // reader cannot satisfy, so it must be rejected as UnknownRequiredBlock.
    let required = ExtraBlock {
        block_type: "token_index",
        required: true,
        codec: "search-codec",
        payload: b"search-index-payload".to_vec(),
    };
    let container = build_container(b"abc\n", None, 0, &[required]);

    assert_eq!(
        QztReader::open(container).map(|_| ()),
        Err(QztError::UnknownRequiredBlock)
    );
}

#[test]
fn duplicate_required_chunk_table_block_is_rejected() {
    // IndexRoot with two required chunk_table blocks must be rejected at open.
    let fake_checksum = Checksum::blake3(b"placeholder");
    let index_root = IndexRoot {
        container_id: CONTAINER_ID,
        blocks: vec![
            BlockDescriptor::chunk_table(128, 64, fake_checksum.clone()),
            BlockDescriptor::chunk_table(192, 64, fake_checksum),
        ],
        original_size: 100,
        original_checksum: Checksum::blake3(b"fake_original"),
        chunk_count: 1,
        line_count: 1,
    };
    let encoded = index_root.encode().expect("index root should encode");

    assert_eq!(
        IndexRoot::decode(&encoded).map(|_| ()),
        Err(QztError::ContainerCorrupt)
    );
}

fn dictionary_block(dictionaries: Vec<DictionaryEntry>) -> DictionaryBlock {
    DictionaryBlock {
        container_id: CONTAINER_ID,
        dictionaries,
    }
}

fn dictionary_entry(
    dictionary_id: u32,
    bytes: Vec<u8>,
    checksum_override: Option<[u8; 32]>,
) -> DictionaryEntry {
    let checksum = checksum_override.map_or_else(
        || Checksum::blake3(&bytes),
        |value| Checksum {
            algorithm: "blake3".to_owned(),
            value,
        },
    );
    DictionaryEntry {
        dictionary_id,
        codec: "zstd".to_owned(),
        bytes,
        checksum,
    }
}

fn build_container(
    input: &[u8],
    dictionary_block: Option<DictionaryBlock>,
    chunk_dictionary_id: u32,
    extra_blocks: &[ExtraBlock],
) -> Vec<u8> {
    let options = ChunkerOptions {
        target_chunk_size: 64 * 1024,
        max_chunk_size: 64 * 1024,
    };
    let plan = plan_chunks(input, options).expect("chunk plan should work");
    let dictionary_bytes = dictionary_block
        .as_ref()
        .and_then(|block| {
            block
                .dictionaries
                .iter()
                .find(|entry| entry.dictionary_id == chunk_dictionary_id)
        })
        .map(|entry| entry.bytes.as_slice());

    let mut prefix = vec![0; HEADER_LEN];

    let mut entries = Vec::new();
    for chunk in &plan.chunks {
        let start =
            usize::try_from(chunk.logical_offset).expect("logical_offset fits in usize in tests");
        let end = start
            + usize::try_from(chunk.uncompressed_size)
                .expect("uncompressed_size fits in usize in tests");
        let uncompressed = &input[start..end];
        let compressed = if let Some(dictionary) = dictionary_bytes {
            encode_with_dictionary(uncompressed, dictionary)
        } else {
            zstd::stream::encode_all(uncompressed, 0).expect("zstd encode should work")
        };
        let physical_offset = prefix.len() as u64;
        prefix.extend_from_slice(&compressed);
        entries.push(ChunkEntry {
            chunk_id: chunk.chunk_id,
            physical_offset,
            compressed_size: compressed.len() as u64,
            logical_offset: chunk.logical_offset,
            uncompressed_size: chunk.uncompressed_size,
            first_line: chunk.first_line,
            line_count: chunk.line_count,
            dictionary_id: chunk_dictionary_id,
            flags: chunk.flags,
            compressed_checksum_blake3: Checksum::blake3(&compressed).value,
            uncompressed_checksum_blake3: Checksum::blake3(uncompressed).value,
        });
    }

    let metadata_offset = prefix.len() as u64;
    let metadata = Metadata::for_source_with_options(
        CONTAINER_ID,
        input.len() as u64,
        Checksum::blake3(input),
        "lf",
        plan.line_count,
        MetadataOptions {
            dictionary_mode: if dictionary_block.is_some() {
                "embedded"
            } else {
                "none"
            },
            ..MetadataOptions::default()
        },
    );
    let metadata_bytes = metadata.encode().expect("metadata should encode");
    prefix.extend_from_slice(&metadata_bytes);

    let mut block_descriptors = Vec::new();
    if let Some(block) = dictionary_block {
        let dictionary_offset = prefix.len() as u64;
        let dictionary_bytes = block.encode().expect("dictionary should encode");
        prefix.extend_from_slice(&dictionary_bytes);
        block_descriptors.push(BlockDescriptor::dictionary(
            dictionary_offset,
            dictionary_bytes.len() as u64,
            Checksum::blake3(&dictionary_bytes),
        ));
    }

    for block in extra_blocks {
        let offset = prefix.len() as u64;
        prefix.extend_from_slice(&block.payload);
        block_descriptors.push(BlockDescriptor {
            block_type: block.block_type.to_owned(),
            required: block.required,
            offset,
            size: block.payload.len() as u64,
            codec: block.codec.to_owned(),
            checksum: Checksum::blake3(&block.payload),
            flags: 0,
        });
    }

    let chunk_table_offset = prefix.len() as u64;
    let mut chunk_table_bytes = Vec::new();
    for entry in &entries {
        chunk_table_bytes.extend_from_slice(&entry.encode());
    }
    prefix.extend_from_slice(&chunk_table_bytes);
    block_descriptors.insert(
        0,
        BlockDescriptor::chunk_table(
            chunk_table_offset,
            chunk_table_bytes.len() as u64,
            Checksum::blake3(&chunk_table_bytes),
        ),
    );

    let index_root_offset = prefix.len() as u64;
    let index_root = IndexRoot {
        container_id: CONTAINER_ID,
        blocks: block_descriptors,
        original_size: input.len() as u64,
        original_checksum: Checksum::blake3(input),
        chunk_count: entries.len() as u64,
        line_count: plan.line_count,
    };
    let index_root_bytes = index_root.encode().expect("index root should encode");
    prefix.extend_from_slice(&index_root_bytes);

    let footer_payload_offset = prefix.len() as u64;
    let index_root_ref = BlockRef {
        offset: index_root_offset,
        size: index_root_bytes.len() as u64,
        checksum: Checksum::blake3(&index_root_bytes),
    };
    let metadata_ref = BlockRef {
        offset: metadata_offset,
        size: metadata_bytes.len() as u64,
        checksum: Checksum::blake3(&metadata_bytes),
    };
    let footer_payload =
        fixed_point_footer_payload(&index_root_ref, &metadata_ref, footer_payload_offset);
    let footer_payload_bytes = footer_payload.encode().expect("footer should encode");
    let footer_trailer = FooterTrailer {
        footer_payload_offset,
        footer_payload_size: footer_payload_bytes.len() as u64,
        footer_payload_checksum_blake3: Checksum::blake3(&footer_payload_bytes).value,
    };

    let header = Header {
        metadata_offset,
        metadata_size: metadata_bytes.len() as u64,
        index_hint_offset: index_root_offset,
        container_id: CONTAINER_ID,
    };
    prefix[..HEADER_LEN].copy_from_slice(&header.encode());
    prefix.extend_from_slice(&footer_payload_bytes);
    prefix.extend_from_slice(&footer_trailer.encode());
    prefix
}

fn encode_with_dictionary(input: &[u8], dictionary: &[u8]) -> Vec<u8> {
    let mut encoder =
        zstd::stream::Encoder::with_dictionary(Vec::new(), 0, dictionary).expect("dict encoder");
    encoder.write_all(input).expect("dict encode write");
    encoder.finish().expect("dict encode finish")
}

fn fixed_point_footer_payload(
    index_root: &BlockRef,
    metadata: &BlockRef,
    footer_payload_offset: u64,
) -> FooterPayload {
    let mut final_file_size = 0_u64;

    for _ in 0..8 {
        let candidate = FooterPayload {
            container_id: CONTAINER_ID,
            index_root: index_root.clone(),
            metadata: metadata.clone(),
            final_file_size,
            footer_flags: 0,
            container_checksum: None,
        };
        let size = candidate.encode().expect("footer candidate").len() as u64;
        let next = footer_payload_offset + size + FOOTER_TRAILER_LEN as u64;
        if next == final_file_size {
            return candidate;
        }
        final_file_size = next;
    }

    panic!("footer payload did not converge")
}
