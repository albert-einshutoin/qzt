/// Reader resource limits for untrusted QZT containers.
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
            max_preview_bytes: 1024 * 1024,
            max_cbor_allocation: 16 * 1024 * 1024,
            max_cbor_items: 1_000_000,
        }
    }
}
