#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    qzt_fuzz::exercise_qzi(data);
});
