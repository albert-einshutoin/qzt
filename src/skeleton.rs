use crate::cbor::{validate_deterministic_with_limits, CborLimits};
use crate::chunk_table::{validate_chunk_table_block, ChunkEntry};
use crate::dense_line_index::DenseLineIndex;
use crate::error::{QztError, Result};
use crate::fixed::{validate_physical_ranges, FooterTrailer, Header, PhysicalRange};
use crate::format::{FOOTER_TRAILER_LEN, HEADER_LEN};
use crate::io::ReadAt;
use crate::limits::ResourceLimits;
use crate::primitives::checked_physical_end;
use crate::schema::{
    validate_source_consistency, BlockDescriptor, BlockRef, Checksum, DictionaryBlock,
    DictionaryEntry, DocumentIndex, FooterPayload, IndexRoot, Metadata,
};

/// Summary returned by the structural skeleton opener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonSummary {
    pub container_id: [u8; 16],
    pub original_size: u64,
    pub chunk_count: u64,
    pub line_count: u64,
}

/// Structural details needed by later reader/export phases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonDetails {
    pub summary: SkeletonSummary,
    pub chunk_entries: Vec<ChunkEntry>,
    pub metadata: Metadata,
    pub footer_payload: FooterPayload,
    pub footer_payload_offset: u64,
    pub dictionaries: Vec<DictionaryEntry>,
    pub dense_line_index: Option<DenseLineIndex>,
    pub document_index: Option<DocumentIndex>,
}

/// Writes an empty, structurally valid QZT Core container skeleton.
pub fn write_empty_container(container_id: [u8; 16]) -> Result<Vec<u8>> {
    let metadata = Metadata::empty(container_id);
    let metadata_bytes = metadata.encode()?;
    let metadata_offset = HEADER_LEN as u64;
    let metadata_size =
        u64::try_from(metadata_bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?;

    let chunk_table_bytes = Vec::new();
    let chunk_table_offset = metadata_offset
        .checked_add(metadata_size)
        .ok_or(QztError::PhysicalRangeOutOfBounds)?;
    let chunk_table_size = 0_u64;

    let chunk_table_checksum = Checksum::blake3(&chunk_table_bytes);
    let index_root = IndexRoot {
        container_id,
        blocks: vec![BlockDescriptor::chunk_table(
            chunk_table_offset,
            chunk_table_size,
            chunk_table_checksum,
        )],
        original_size: metadata.original_size,
        original_checksum: metadata.original_checksum.clone(),
        chunk_count: 0,
        line_count: metadata.line_count,
    };
    let index_root_bytes = index_root.encode()?;
    let index_root_offset = chunk_table_offset
        .checked_add(chunk_table_size)
        .ok_or(QztError::PhysicalRangeOutOfBounds)?;
    let index_root_size =
        u64::try_from(index_root_bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?;

    let footer_payload_offset = index_root_offset
        .checked_add(index_root_size)
        .ok_or(QztError::PhysicalRangeOutOfBounds)?;
    let footer_payload = fixed_point_footer_payload(
        container_id,
        BlockRef {
            offset: index_root_offset,
            size: index_root_size,
            checksum: Checksum::blake3(&index_root_bytes),
        },
        BlockRef {
            offset: metadata_offset,
            size: metadata_size,
            checksum: Checksum::blake3(&metadata_bytes),
        },
        footer_payload_offset,
    )?;

    let footer_payload_bytes = footer_payload.encode()?;
    let footer_trailer = FooterTrailer {
        footer_payload_offset,
        footer_payload_size: u64::try_from(footer_payload_bytes.len())
            .map_err(|_| QztError::ResourceLimitExceeded)?,
        footer_payload_checksum_blake3: Checksum::blake3(&footer_payload_bytes).value,
    };

    let header = Header {
        metadata_offset,
        metadata_size,
        index_hint_offset: index_root_offset,
        container_id,
    };

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&header.encode());
    bytes.extend_from_slice(&metadata_bytes);
    bytes.extend_from_slice(&chunk_table_bytes);
    bytes.extend_from_slice(&index_root_bytes);
    bytes.extend_from_slice(&footer_payload_bytes);
    bytes.extend_from_slice(&footer_trailer.encode());
    Ok(bytes)
}

/// Opens a QZT skeleton through Footer Payload, Metadata, Index Root, and Chunk Table validation.
pub fn open_skeleton(bytes: &[u8]) -> Result<SkeletonSummary> {
    Ok(open_skeleton_details(bytes)?.summary)
}

/// Opens a QZT skeleton and returns structural details through Chunk Table validation.
pub fn open_skeleton_details(bytes: &[u8]) -> Result<SkeletonDetails> {
    open_skeleton_details_with_limits(bytes, ResourceLimits::default())
}

/// Opens a QZT skeleton and returns structural details with explicit resource limits.
pub fn open_skeleton_details_with_limits(
    bytes: &[u8],
    limits: ResourceLimits,
) -> Result<SkeletonDetails> {
    if bytes.len() < HEADER_LEN {
        return Err(QztError::InvalidHeader);
    }
    if bytes.len() < HEADER_LEN + FOOTER_TRAILER_LEN {
        return Err(QztError::InvalidFooterTrailer);
    }

    let final_file_size =
        u64::try_from(bytes.len()).map_err(|_| QztError::ResourceLimitExceeded)?;
    let header = Header::decode(&bytes[..HEADER_LEN])?;
    let trailer = FooterTrailer::decode(&bytes[bytes.len() - FOOTER_TRAILER_LEN..])?;

    let footer_payload_bytes = slice_physical(
        bytes,
        PhysicalRange::new(trailer.footer_payload_offset, trailer.footer_payload_size),
    )?;
    if Checksum::blake3(footer_payload_bytes).value != trailer.footer_payload_checksum_blake3 {
        return Err(QztError::FooterChecksumMismatch);
    }

    validate_deterministic_with_limits(footer_payload_bytes, cbor_limits(limits))?;
    let footer_payload = FooterPayload::decode(footer_payload_bytes)?;
    if footer_payload.final_file_size != final_file_size {
        return Err(QztError::FinalFileSizeMismatch);
    }
    if footer_payload.container_id != header.container_id {
        return Err(QztError::ContainerIdMismatch);
    }
    if footer_payload.metadata.offset != header.metadata_offset
        || footer_payload.metadata.size != header.metadata_size
    {
        return Err(QztError::MetadataInvalid);
    }

    enforce_index_block_size(footer_payload.metadata.size, limits)?;
    let metadata_bytes = slice_block_ref(bytes, &footer_payload.metadata)?;
    if Checksum::blake3(metadata_bytes) != footer_payload.metadata.checksum {
        return Err(QztError::MetadataChecksumMismatch);
    }
    validate_deterministic_with_limits(metadata_bytes, cbor_limits(limits))?;
    let metadata = Metadata::decode(metadata_bytes)?;
    if metadata.container_id != header.container_id {
        return Err(QztError::ContainerIdMismatch);
    }

    enforce_index_block_size(footer_payload.index_root.size, limits)?;
    let index_root_bytes = slice_block_ref(bytes, &footer_payload.index_root)?;
    if Checksum::blake3(index_root_bytes) != footer_payload.index_root.checksum {
        return Err(QztError::IndexRootChecksumMismatch);
    }
    validate_deterministic_with_limits(index_root_bytes, cbor_limits(limits))?;
    let index_root = IndexRoot::decode(index_root_bytes)?;
    validate_source_consistency(&metadata, &index_root)?;

    let chunk_table = index_root.chunk_table_block()?;
    if chunk_table.codec != "qzt-ctbl-fixed-v1" {
        return Err(QztError::ChunkTableInvalid);
    }
    for block in &index_root.blocks {
        enforce_index_block_size(block.size, limits)?;
    }

    let chunk_table_bytes = slice_physical(
        bytes,
        PhysicalRange::new(chunk_table.offset, chunk_table.size),
    )?;
    if Checksum::blake3(chunk_table_bytes) != chunk_table.checksum {
        return Err(QztError::ChunkTableChecksumMismatch);
    }

    let chunk_entries = validate_chunk_table_block(
        chunk_table_bytes,
        index_root.chunk_count,
        index_root.original_size,
        index_root.line_count,
    )?;
    for entry in &chunk_entries {
        if entry.uncompressed_size > limits.max_uncompressed_chunk_size {
            return Err(QztError::ResourceLimitExceeded);
        }
    }

    let mut ranges = vec![
        PhysicalRange::new(header.metadata_offset, header.metadata_size),
        PhysicalRange::new(
            footer_payload.index_root.offset,
            footer_payload.index_root.size,
        ),
        PhysicalRange::new(trailer.footer_payload_offset, trailer.footer_payload_size),
        PhysicalRange::new(chunk_table.offset, chunk_table.size),
    ];
    ranges.extend(index_root.blocks.iter().filter_map(|block| {
        (!matches!(block.block_type.as_str(), "metadata" | "chunk_table"))
            .then_some(PhysicalRange::new(block.offset, block.size))
    }));
    ranges.extend(
        chunk_entries
            .iter()
            .map(|entry| PhysicalRange::new(entry.physical_offset, entry.compressed_size)),
    );
    validate_physical_ranges(final_file_size, &ranges)?;

    let dictionaries = parse_dictionary_blocks(bytes, &index_root, header.container_id, limits)?;
    validate_required_dictionaries(&chunk_entries, &dictionaries)?;
    let dense_line_index = parse_dense_line_index(bytes, &index_root, &chunk_entries)?;
    let document_index = parse_document_index(bytes, &index_root, header.container_id, limits)?;

    Ok(SkeletonDetails {
        summary: SkeletonSummary {
            container_id: header.container_id,
            original_size: index_root.original_size,
            chunk_count: index_root.chunk_count,
            line_count: index_root.line_count,
        },
        chunk_entries,
        metadata,
        footer_payload,
        footer_payload_offset: trailer.footer_payload_offset,
        dictionaries,
        dense_line_index,
        document_index,
    })
}

/// Opens structural details from a positioned reader without reading chunk data.
pub fn open_skeleton_details_read_at<R: ReadAt>(
    reader: &R,
    final_file_size: u64,
    limits: ResourceLimits,
) -> Result<SkeletonDetails> {
    if final_file_size < HEADER_LEN as u64 {
        return Err(QztError::InvalidHeader);
    }
    if final_file_size < (HEADER_LEN + FOOTER_TRAILER_LEN) as u64 {
        return Err(QztError::InvalidFooterTrailer);
    }

    let mut header_bytes = vec![0_u8; HEADER_LEN];
    read_exact_at_qzt(reader, 0, &mut header_bytes)?;
    let header = Header::decode(&header_bytes)?;

    let trailer_offset = final_file_size
        .checked_sub(FOOTER_TRAILER_LEN as u64)
        .ok_or(QztError::InvalidFooterTrailer)?;
    let mut trailer_bytes = vec![0_u8; FOOTER_TRAILER_LEN];
    read_exact_at_qzt(reader, trailer_offset, &mut trailer_bytes)?;
    let trailer = FooterTrailer::decode(&trailer_bytes)?;

    let footer_payload_bytes = read_physical_at(
        reader,
        final_file_size,
        PhysicalRange::new(trailer.footer_payload_offset, trailer.footer_payload_size),
    )?;
    if Checksum::blake3(&footer_payload_bytes).value != trailer.footer_payload_checksum_blake3 {
        return Err(QztError::FooterChecksumMismatch);
    }

    validate_deterministic_with_limits(&footer_payload_bytes, cbor_limits(limits))?;
    let footer_payload = FooterPayload::decode(&footer_payload_bytes)?;
    if footer_payload.final_file_size != final_file_size {
        return Err(QztError::FinalFileSizeMismatch);
    }
    if footer_payload.container_id != header.container_id {
        return Err(QztError::ContainerIdMismatch);
    }
    if footer_payload.metadata.offset != header.metadata_offset
        || footer_payload.metadata.size != header.metadata_size
    {
        return Err(QztError::MetadataInvalid);
    }

    enforce_index_block_size(footer_payload.metadata.size, limits)?;
    let metadata_bytes = read_block_ref_at(reader, final_file_size, &footer_payload.metadata)?;
    if Checksum::blake3(&metadata_bytes) != footer_payload.metadata.checksum {
        return Err(QztError::MetadataChecksumMismatch);
    }
    validate_deterministic_with_limits(&metadata_bytes, cbor_limits(limits))?;
    let metadata = Metadata::decode(&metadata_bytes)?;
    if metadata.container_id != header.container_id {
        return Err(QztError::ContainerIdMismatch);
    }

    enforce_index_block_size(footer_payload.index_root.size, limits)?;
    let index_root_bytes = read_block_ref_at(reader, final_file_size, &footer_payload.index_root)?;
    if Checksum::blake3(&index_root_bytes) != footer_payload.index_root.checksum {
        return Err(QztError::IndexRootChecksumMismatch);
    }
    validate_deterministic_with_limits(&index_root_bytes, cbor_limits(limits))?;
    let index_root = IndexRoot::decode(&index_root_bytes)?;
    validate_source_consistency(&metadata, &index_root)?;

    let chunk_table = index_root.chunk_table_block()?;
    if chunk_table.codec != "qzt-ctbl-fixed-v1" {
        return Err(QztError::ChunkTableInvalid);
    }
    for block in &index_root.blocks {
        enforce_index_block_size(block.size, limits)?;
    }

    let chunk_table_bytes = read_physical_at(
        reader,
        final_file_size,
        PhysicalRange::new(chunk_table.offset, chunk_table.size),
    )?;
    if Checksum::blake3(&chunk_table_bytes) != chunk_table.checksum {
        return Err(QztError::ChunkTableChecksumMismatch);
    }

    let chunk_entries = validate_chunk_table_block(
        &chunk_table_bytes,
        index_root.chunk_count,
        index_root.original_size,
        index_root.line_count,
    )?;
    for entry in &chunk_entries {
        if entry.uncompressed_size > limits.max_uncompressed_chunk_size {
            return Err(QztError::ResourceLimitExceeded);
        }
    }

    let mut ranges = vec![
        PhysicalRange::new(header.metadata_offset, header.metadata_size),
        PhysicalRange::new(
            footer_payload.index_root.offset,
            footer_payload.index_root.size,
        ),
        PhysicalRange::new(trailer.footer_payload_offset, trailer.footer_payload_size),
        PhysicalRange::new(chunk_table.offset, chunk_table.size),
    ];
    ranges.extend(index_root.blocks.iter().filter_map(|block| {
        (!matches!(block.block_type.as_str(), "metadata" | "chunk_table"))
            .then_some(PhysicalRange::new(block.offset, block.size))
    }));
    ranges.extend(
        chunk_entries
            .iter()
            .map(|entry| PhysicalRange::new(entry.physical_offset, entry.compressed_size)),
    );
    validate_physical_ranges(final_file_size, &ranges)?;

    let dictionaries = parse_dictionary_blocks_at(
        reader,
        final_file_size,
        &index_root,
        header.container_id,
        limits,
    )?;
    validate_required_dictionaries(&chunk_entries, &dictionaries)?;
    let dense_line_index =
        parse_dense_line_index_at(reader, final_file_size, &index_root, &chunk_entries)?;
    let document_index = parse_document_index_at(
        reader,
        final_file_size,
        &index_root,
        header.container_id,
        limits,
    )?;

    Ok(SkeletonDetails {
        summary: SkeletonSummary {
            container_id: header.container_id,
            original_size: index_root.original_size,
            chunk_count: index_root.chunk_count,
            line_count: index_root.line_count,
        },
        chunk_entries,
        metadata,
        footer_payload,
        footer_payload_offset: trailer.footer_payload_offset,
        dictionaries,
        dense_line_index,
        document_index,
    })
}

fn enforce_index_block_size(size: u64, limits: ResourceLimits) -> Result<()> {
    if size > limits.max_index_block_size {
        return Err(QztError::ResourceLimitExceeded);
    }
    Ok(())
}

fn cbor_limits(limits: ResourceLimits) -> CborLimits {
    CborLimits {
        max_allocation: limits.max_cbor_allocation,
        max_items: limits.max_cbor_items,
    }
}

fn parse_dictionary_blocks(
    bytes: &[u8],
    index_root: &IndexRoot,
    container_id: [u8; 16],
    limits: ResourceLimits,
) -> Result<Vec<DictionaryEntry>> {
    let mut dictionaries = Vec::new();

    for descriptor in index_root
        .blocks
        .iter()
        .filter(|block| block.block_type == "dictionary")
    {
        if descriptor.required || descriptor.codec != "qzt-dict-cbor-v1" {
            return Err(QztError::ContainerCorrupt);
        }

        enforce_index_block_size(descriptor.size, limits)?;
        let dictionary_bytes = slice_physical(
            bytes,
            PhysicalRange::new(descriptor.offset, descriptor.size),
        )?;
        if Checksum::blake3(dictionary_bytes) != descriptor.checksum {
            return Err(QztError::DictionaryChecksumMismatch);
        }

        validate_deterministic_with_limits(dictionary_bytes, cbor_limits(limits))?;
        let block =
            DictionaryBlock::decode_with_limits(dictionary_bytes, limits.max_dictionary_size)?;
        if block.container_id != container_id {
            return Err(QztError::ContainerIdMismatch);
        }

        for entry in block.dictionaries {
            if dictionaries
                .iter()
                .any(|existing: &DictionaryEntry| existing.dictionary_id == entry.dictionary_id)
            {
                return Err(QztError::ContainerCorrupt);
            }
            dictionaries.push(entry);
        }
    }

    Ok(dictionaries)
}

fn validate_required_dictionaries(
    chunk_entries: &[ChunkEntry],
    dictionaries: &[DictionaryEntry],
) -> Result<()> {
    for entry in chunk_entries {
        if entry.dictionary_id == 0 {
            continue;
        }

        if !dictionaries
            .iter()
            .any(|dictionary| dictionary.dictionary_id == entry.dictionary_id)
        {
            return Err(QztError::MissingDictionary);
        }
    }

    Ok(())
}

fn parse_dense_line_index(
    bytes: &[u8],
    index_root: &IndexRoot,
    chunk_entries: &[ChunkEntry],
) -> Result<Option<DenseLineIndex>> {
    let mut dense = None;

    for descriptor in index_root
        .blocks
        .iter()
        .filter(|block| block.block_type == "dense_line_index")
    {
        if descriptor.required || descriptor.codec != "qzt-line-delta-varint-v1" {
            return Err(QztError::ChunkTableInvalid);
        }
        if dense.is_some() {
            return Err(QztError::ChunkTableInvalid);
        }

        let dense_bytes = slice_physical(
            bytes,
            PhysicalRange::new(descriptor.offset, descriptor.size),
        )?;
        if Checksum::blake3(dense_bytes) != descriptor.checksum {
            return Err(QztError::ChunkTableChecksumMismatch);
        }
        dense = Some(DenseLineIndex::decode_for_chunks(
            dense_bytes,
            chunk_entries,
        )?);
    }

    Ok(dense)
}

fn parse_document_index(
    bytes: &[u8],
    index_root: &IndexRoot,
    container_id: [u8; 16],
    limits: ResourceLimits,
) -> Result<Option<DocumentIndex>> {
    let mut document_index = None;

    for descriptor in index_root
        .blocks
        .iter()
        .filter(|block| block.block_type == "document_index")
    {
        if descriptor.required || descriptor.codec != "qzt-doc-index-cbor-v1" {
            return Err(QztError::ContainerCorrupt);
        }
        if document_index.is_some() {
            return Err(QztError::ContainerCorrupt);
        }

        let document_bytes = slice_physical(
            bytes,
            PhysicalRange::new(descriptor.offset, descriptor.size),
        )?;
        if Checksum::blake3(document_bytes) != descriptor.checksum {
            return Err(QztError::ContainerCorrupt);
        }

        validate_deterministic_with_limits(document_bytes, cbor_limits(limits))?;
        let block = DocumentIndex::decode(document_bytes)?;
        if block.container_id != container_id {
            return Err(QztError::ContainerIdMismatch);
        }
        document_index = Some(block);
    }

    Ok(document_index)
}

fn parse_dictionary_blocks_at<R: ReadAt>(
    reader: &R,
    final_file_size: u64,
    index_root: &IndexRoot,
    container_id: [u8; 16],
    limits: ResourceLimits,
) -> Result<Vec<DictionaryEntry>> {
    let mut dictionaries = Vec::new();

    for descriptor in index_root
        .blocks
        .iter()
        .filter(|block| block.block_type == "dictionary")
    {
        if descriptor.required || descriptor.codec != "qzt-dict-cbor-v1" {
            return Err(QztError::ContainerCorrupt);
        }

        enforce_index_block_size(descriptor.size, limits)?;
        let dictionary_bytes = read_physical_at(
            reader,
            final_file_size,
            PhysicalRange::new(descriptor.offset, descriptor.size),
        )?;
        if Checksum::blake3(&dictionary_bytes) != descriptor.checksum {
            return Err(QztError::DictionaryChecksumMismatch);
        }

        validate_deterministic_with_limits(&dictionary_bytes, cbor_limits(limits))?;
        let block =
            DictionaryBlock::decode_with_limits(&dictionary_bytes, limits.max_dictionary_size)?;
        if block.container_id != container_id {
            return Err(QztError::ContainerIdMismatch);
        }

        for entry in block.dictionaries {
            if dictionaries
                .iter()
                .any(|existing: &DictionaryEntry| existing.dictionary_id == entry.dictionary_id)
            {
                return Err(QztError::ContainerCorrupt);
            }
            dictionaries.push(entry);
        }
    }

    Ok(dictionaries)
}

fn parse_dense_line_index_at<R: ReadAt>(
    reader: &R,
    final_file_size: u64,
    index_root: &IndexRoot,
    chunk_entries: &[ChunkEntry],
) -> Result<Option<DenseLineIndex>> {
    let mut dense = None;

    for descriptor in index_root
        .blocks
        .iter()
        .filter(|block| block.block_type == "dense_line_index")
    {
        if descriptor.required || descriptor.codec != "qzt-line-delta-varint-v1" {
            return Err(QztError::ChunkTableInvalid);
        }
        if dense.is_some() {
            return Err(QztError::ChunkTableInvalid);
        }

        let dense_bytes = read_physical_at(
            reader,
            final_file_size,
            PhysicalRange::new(descriptor.offset, descriptor.size),
        )?;
        if Checksum::blake3(&dense_bytes) != descriptor.checksum {
            return Err(QztError::ChunkTableChecksumMismatch);
        }
        dense = Some(DenseLineIndex::decode_for_chunks(
            &dense_bytes,
            chunk_entries,
        )?);
    }

    Ok(dense)
}

fn parse_document_index_at<R: ReadAt>(
    reader: &R,
    final_file_size: u64,
    index_root: &IndexRoot,
    container_id: [u8; 16],
    limits: ResourceLimits,
) -> Result<Option<DocumentIndex>> {
    let mut document_index = None;

    for descriptor in index_root
        .blocks
        .iter()
        .filter(|block| block.block_type == "document_index")
    {
        if descriptor.required || descriptor.codec != "qzt-doc-index-cbor-v1" {
            return Err(QztError::ContainerCorrupt);
        }
        if document_index.is_some() {
            return Err(QztError::ContainerCorrupt);
        }

        let document_bytes = read_physical_at(
            reader,
            final_file_size,
            PhysicalRange::new(descriptor.offset, descriptor.size),
        )?;
        if Checksum::blake3(&document_bytes) != descriptor.checksum {
            return Err(QztError::ContainerCorrupt);
        }

        validate_deterministic_with_limits(&document_bytes, cbor_limits(limits))?;
        let block = DocumentIndex::decode(&document_bytes)?;
        if block.container_id != container_id {
            return Err(QztError::ContainerIdMismatch);
        }
        document_index = Some(block);
    }

    Ok(document_index)
}

fn fixed_point_footer_payload(
    container_id: [u8; 16],
    index_root: BlockRef,
    metadata: BlockRef,
    footer_payload_offset: u64,
) -> Result<FooterPayload> {
    let mut final_file_size = 0_u64;

    for _ in 0..8 {
        let candidate = FooterPayload {
            container_id,
            index_root: index_root.clone(),
            metadata: metadata.clone(),
            final_file_size,
            footer_flags: 0,
            container_checksum: None,
        };
        let size = u64::try_from(candidate.encode()?.len())
            .map_err(|_| QztError::ResourceLimitExceeded)?;
        let next = footer_payload_offset
            .checked_add(size)
            .and_then(|value| value.checked_add(FOOTER_TRAILER_LEN as u64))
            .ok_or(QztError::PhysicalRangeOutOfBounds)?;

        if next == final_file_size {
            return Ok(candidate);
        }

        final_file_size = next;
    }

    Err(QztError::ContainerCorrupt)
}

fn slice_block_ref<'a>(bytes: &'a [u8], block: &BlockRef) -> Result<&'a [u8]> {
    slice_physical(bytes, PhysicalRange::new(block.offset, block.size))
}

fn read_block_ref_at<R: ReadAt>(
    reader: &R,
    final_file_size: u64,
    block: &BlockRef,
) -> Result<Vec<u8>> {
    read_physical_at(
        reader,
        final_file_size,
        PhysicalRange::new(block.offset, block.size),
    )
}

fn read_physical_at<R: ReadAt>(
    reader: &R,
    final_file_size: u64,
    range: PhysicalRange,
) -> Result<Vec<u8>> {
    let end = checked_physical_end(range.offset, range.size)?;
    if end > final_file_size {
        return Err(QztError::PhysicalRangeOutOfBounds);
    }
    let len = usize::try_from(range.size).map_err(|_| QztError::ResourceLimitExceeded)?;
    let mut bytes = vec![0_u8; len];
    read_exact_at_qzt(reader, range.offset, &mut bytes)?;
    Ok(bytes)
}

fn read_exact_at_qzt<R: ReadAt>(reader: &R, offset: u64, buf: &mut [u8]) -> Result<()> {
    reader
        .read_exact_at(offset, buf)
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::UnexpectedEof => QztError::UnexpectedEof,
            _ => QztError::ContainerCorrupt,
        })
}

fn slice_physical(bytes: &[u8], range: PhysicalRange) -> Result<&[u8]> {
    let end = checked_physical_end(range.offset, range.size)?;
    if end > bytes.len() as u64 {
        return Err(QztError::PhysicalRangeOutOfBounds);
    }
    let start = usize::try_from(range.offset).map_err(|_| QztError::PhysicalRangeOutOfBounds)?;
    let end = usize::try_from(end).map_err(|_| QztError::PhysicalRangeOutOfBounds)?;
    bytes
        .get(start..end)
        .ok_or(QztError::PhysicalRangeOutOfBounds)
}
