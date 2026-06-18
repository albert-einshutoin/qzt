//! Property-based round-trip coverage for the writer/reader.
//!
//! Example-based conformance tests enumerate known cases; these properties probe
//! the UTF-8 / CRLF / chunk-boundary edges that are hard to enumerate by hand.

use proptest::prelude::*;
use qzt::{ChunkerOptions, QztReader, VerifyLevel, WriterOptions, pack_bytes};

/// Small chunk sizes so arbitrary inputs cross several chunk boundaries.
fn small_chunk_options() -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size: 8,
            max_chunk_size: 32,
        },
        zstd_level: 0,
    }
}

proptest! {
    /// Packing then exporting reproduces the original bytes exactly, and the
    /// resulting container passes deep verification.
    #[test]
    fn export_after_pack_is_identity(input in any::<String>()) {
        let bytes = input.as_bytes();
        let container = pack_bytes(bytes, small_chunk_options())
            .expect("valid UTF-8 input should pack");
        let reader = QztReader::open(&container).expect("packed container should open");

        prop_assert_eq!(reader.export_all().expect("export"), bytes);
        prop_assert!(reader.verify(VerifyLevel::Deep).is_ok());
    }

    /// A byte range read returns exactly the corresponding slice of the original.
    #[test]
    fn read_range_matches_original_slice(
        input in any::<String>(),
        x in any::<u64>(),
        y in any::<u64>(),
    ) {
        let bytes = input.as_bytes();
        let len = bytes.len() as u64;
        let container = pack_bytes(bytes, small_chunk_options())
            .expect("valid UTF-8 input should pack");
        let reader = QztReader::open(&container).expect("packed container should open");

        let offset = if len == 0 { 0 } else { x % (len + 1) };
        let length = y % (len - offset + 1);
        let start = usize::try_from(offset).expect("offset fits in tests");
        let end = usize::try_from(offset + length).expect("offset+length fits in tests");

        prop_assert_eq!(
            reader.read_range(offset, length).expect("range read"),
            &bytes[start..end]
        );
    }
}
