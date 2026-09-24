use crate::cbor::CborLimits;
use crate::error::{QztError, Result};

/// Reader resource limits for untrusted QZT containers and successful Writer output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    /// Maximum compressed bytes read into memory for one chunk decode.
    pub max_compressed_chunk_size: u64,
    /// Maximum decoded bytes accepted for one chunk.
    pub max_uncompressed_chunk_size: u64,
    /// Maximum bytes accepted for one embedded zstd dictionary.
    pub max_dictionary_size: u64,
    /// Maximum bytes accepted for one index block.
    pub max_index_block_size: u64,
    /// Maximum cumulative bytes requested for decoded Dense Line Index entry
    /// and offset vectors. Stored index block bytes have a separate limit.
    pub max_dense_line_index_allocation: u64,
    /// Maximum bytes exposed by preview-oriented operations.
    pub max_preview_bytes: u64,
    /// Maximum aggregate bytes allocated while decoding one CBOR value.
    ///
    /// This covers byte/text payloads and canonical map-key copies. CBOR nesting
    /// is independently capped at 64 levels to protect the native stack.
    pub max_cbor_allocation: u64,
    /// Maximum aggregate CBOR values decoded from one CBOR item.
    ///
    /// The root value, container values, map keys, and map values all count.
    pub max_cbor_items: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            // Why 72 MiB: the default 64 MiB decoded limit needs modest room for
            // zstd framing and worst-case incompressible overhead, while still
            // preventing attacker-controlled chunk tables from requesting an
            // effectively unbounded allocation.
            max_compressed_chunk_size: 72 * 1024 * 1024,
            max_uncompressed_chunk_size: 64 * 1024 * 1024,
            max_dictionary_size: 16 * 1024 * 1024,
            max_index_block_size: 64 * 1024 * 1024,
            // A 64 MiB encoded block can expand substantially when varint
            // offsets become u64s; cap requested vector storage at 256 MiB.
            max_dense_line_index_allocation: 256 * 1024 * 1024,
            max_preview_bytes: 1024 * 1024,
            max_cbor_allocation: 16 * 1024 * 1024,
            max_cbor_items: 1_000_000,
        }
    }
}

impl ResourceLimits {
    pub(crate) fn enforce_index_block_size(self, size: u64) -> Result<()> {
        if size > self.max_index_block_size {
            return Err(QztError::ResourceLimitExceeded);
        }
        Ok(())
    }

    pub(crate) fn enforce_chunk_table_entries(self, count: usize) -> Result<()> {
        let bytes = count.checked_mul(crate::chunk_table::CHUNK_ENTRY_LEN)
            .ok_or(QztError::ResourceLimitExceeded)?;
        self.enforce_index_block_size(
            u64::try_from(bytes).map_err(|_| QztError::ResourceLimitExceeded)?,
        )
    }

    pub(crate) fn enforce_chunk_sizes(self, compressed: u64, uncompressed: u64) -> Result<()> {
        if compressed > self.max_compressed_chunk_size
            || uncompressed > self.max_uncompressed_chunk_size
        {
            return Err(QztError::ResourceLimitExceeded);
        }
        Ok(())
    }

    pub(crate) fn enforce_dense_allocation(self, bytes: u64) -> Result<()> {
        if bytes > self.max_dense_line_index_allocation {
            return Err(QztError::ResourceLimitExceeded);
        }
        Ok(())
    }

    pub(crate) fn cbor_limits(self) -> CborLimits {
        CborLimits {
            max_allocation: self.max_cbor_allocation,
            max_items: self.max_cbor_items,
        }
    }
}
