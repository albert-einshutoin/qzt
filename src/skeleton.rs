use crate::chunk_table::validate_chunk_table_block;
use crate::error::{QztError, Result};
use crate::fixed::{validate_physical_ranges, FooterTrailer, Header, PhysicalRange};
use crate::format::{FOOTER_TRAILER_LEN, HEADER_LEN};
use crate::primitives::checked_physical_end;
use crate::schema::{
    validate_source_consistency, BlockDescriptor, BlockRef, Checksum, FooterPayload, IndexRoot,
    Metadata,
};

/// Summary returned by the structural skeleton opener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonSummary {
    pub container_id: [u8; 16],
    pub original_size: u64,
    pub chunk_count: u64,
    pub line_count: u64,
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

    let metadata_bytes = slice_block_ref(bytes, &footer_payload.metadata)?;
    if Checksum::blake3(metadata_bytes) != footer_payload.metadata.checksum {
        return Err(QztError::MetadataChecksumMismatch);
    }
    let metadata = Metadata::decode(metadata_bytes)?;
    if metadata.container_id != header.container_id {
        return Err(QztError::ContainerIdMismatch);
    }

    let index_root_bytes = slice_block_ref(bytes, &footer_payload.index_root)?;
    if Checksum::blake3(index_root_bytes) != footer_payload.index_root.checksum {
        return Err(QztError::IndexRootChecksumMismatch);
    }
    let index_root = IndexRoot::decode(index_root_bytes)?;
    validate_source_consistency(&metadata, &index_root)?;

    let chunk_table = index_root.chunk_table_block()?;
    if chunk_table.codec != "qzt-ctbl-fixed-v1" {
        return Err(QztError::ChunkTableInvalid);
    }

    validate_physical_ranges(
        final_file_size,
        &[
            PhysicalRange::new(header.metadata_offset, header.metadata_size),
            PhysicalRange::new(
                footer_payload.index_root.offset,
                footer_payload.index_root.size,
            ),
            PhysicalRange::new(trailer.footer_payload_offset, trailer.footer_payload_size),
            PhysicalRange::new(chunk_table.offset, chunk_table.size),
        ],
    )?;

    let chunk_table_bytes = slice_physical(
        bytes,
        PhysicalRange::new(chunk_table.offset, chunk_table.size),
    )?;
    if Checksum::blake3(chunk_table_bytes) != chunk_table.checksum {
        return Err(QztError::ChunkTableChecksumMismatch);
    }

    validate_chunk_table_block(
        chunk_table_bytes,
        index_root.chunk_count,
        index_root.original_size,
        index_root.line_count,
    )?;

    Ok(SkeletonSummary {
        container_id: header.container_id,
        original_size: index_root.original_size,
        chunk_count: index_root.chunk_count,
        line_count: index_root.line_count,
    })
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
