//! Property-based round-trip coverage for the writer/reader.
//!
//! Example-based conformance tests enumerate known cases; these properties probe
//! the UTF-8 / CRLF / chunk-boundary edges that are hard to enumerate by hand.

use proptest::prelude::*;
use qzt::{QztFileReader, QztFileWriter, QztReader, VerifyLevel, pack_bytes};
use std::io::Cursor;
mod support;
use support::small_chunk_options;

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

    /// Fragment boundaries, including inside UTF-8 and CRLF, do not change
    /// the successful bytes or either default Reader's deep verification.
    #[test]
    fn fragmented_stream_matches_memory_pack_and_both_readers(
        input in any::<String>(), step in 1usize..17,
    ) {
        let bytes = input.as_bytes();
        let options = small_chunk_options();
        let expected = pack_bytes(bytes, options).expect("memory pack");
        let mut writer = QztFileWriter::new(Cursor::new(Vec::new()), options).expect("empty sink");
        for fragment in bytes.chunks(step) {
            writer.push(fragment).expect("fragment");
        }
        writer.finish().expect("finish");
        let actual = writer.into_inner().into_inner();
        prop_assert_eq!(&actual, &expected);
        let memory = QztReader::open(&actual).expect("memory open");
        let file = QztFileReader::open_read_at(actual.as_slice(), actual.len() as u64).expect("file open");
        prop_assert!(memory.verify(VerifyLevel::Deep).is_ok());
        prop_assert!(file.verify(VerifyLevel::Deep).is_ok());
        prop_assert_eq!(memory.export_all().expect("memory export"), bytes);
        prop_assert_eq!(file.export_all().expect("file export"), bytes);
    }
}
