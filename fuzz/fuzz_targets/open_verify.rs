#![no_main]

use libfuzzer_sys::fuzz_target;
use qzt::reader::{QztReader, VerifyLevel};
use qzt::writer::{pack_bytes, WriterOptions};

fuzz_target!(|data: &[u8]| {
    if let Ok(reader) = QztReader::open(data) {
        let _ = reader.verify(VerifyLevel::Quick);
        let _ = reader.verify(VerifyLevel::Normal);
        let _ = reader.verify(VerifyLevel::Deep);
    }

    if data.len() <= 1 << 20 && std::str::from_utf8(data).is_ok() {
        if let Ok(container) = pack_bytes(data, WriterOptions::default()) {
            if let Ok(reader) = QztReader::open(&container) {
                let _ = reader.verify(VerifyLevel::Deep);
            }
        }
    }
});
