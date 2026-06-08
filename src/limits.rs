/// Reader resource limits for untrusted QZT containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    pub max_uncompressed_chunk_size: u64,
    pub max_dictionary_size: u64,
    pub max_index_block_size: u64,
    pub max_preview_bytes: u64,
    pub max_cbor_allocation: u64,
    pub max_cbor_items: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_uncompressed_chunk_size: 64 * 1024 * 1024,
            max_dictionary_size: 16 * 1024 * 1024,
            max_index_block_size: 64 * 1024 * 1024,
            max_preview_bytes: 1024 * 1024,
            max_cbor_allocation: 16 * 1024 * 1024,
            max_cbor_items: 1_000_000,
        }
    }
}
