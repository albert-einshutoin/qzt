#![no_main]

use libfuzzer_sys::fuzz_target;
use qzt::{QztReader, VerifyLevel, WriterOptions, pack_bytes};

// Round-trip packing is deliberately bounded below the parser-only fuzz path:
// compression under sanitizers can make one large input outlive the CI smoke
// deadline, preventing libFuzzer from observing its own total-time limit.
const MAX_ROUND_TRIP_BYTES: usize = 4 * 1024;

fuzz_target!(|data: &[u8]| {
    if let Ok(reader) = QztReader::open(data) {
        let _ = reader.verify(VerifyLevel::Quick);
        let _ = reader.verify(VerifyLevel::Normal);
        let _ = reader.verify(VerifyLevel::Deep);
    }

    if data.len() <= MAX_ROUND_TRIP_BYTES && std::str::from_utf8(data).is_ok() {
        if let Ok(container) = pack_bytes(data, WriterOptions::default()) {
            if let Ok(reader) = QztReader::open(&container) {
                let _ = reader.verify(VerifyLevel::Deep);
            }
        }
    }
});
