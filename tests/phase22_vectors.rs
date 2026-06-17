use qzt::reader::{QztReader, VerifyLevel};
use qzt::writer::{WriterOptions, pack_bytes};

#[test]
fn portable_vector_runner_matches_manifest() {
    let manifest = include_str!("vectors/manifest.tsv");
    for line in manifest.lines().skip(1) {
        let fields = line.split('\t').collect::<Vec<_>>();
        let name = fields[0];
        let expect_open = fields[2];
        let expect_deep = fields[3];
        let expect_export = fields[4].replace("\\n", "\n");
        let bytes = decode_hex(vector_hex(name)).expect("vector hex should decode");

        match expect_open {
            "ok" => {
                let reader = QztReader::open(&bytes).expect("vector should open");
                if expect_deep == "ok" {
                    reader
                        .verify(VerifyLevel::Deep)
                        .expect("deep verify should pass");
                }
                assert_eq!(
                    reader.export_all().expect("export"),
                    expect_export.as_bytes()
                );
            }
            "err" => assert!(QztReader::open(&bytes).is_err()),
            other => panic!("unknown expectation {other}"),
        }
    }
}

#[test]
fn vectors_regenerate_byte_identically() {
    assert_eq!(decode_hex(vector_hex("valid_c1")).unwrap(), valid_c1());
    assert_eq!(
        decode_hex(vector_hex("valid_empty")).unwrap(),
        valid_empty()
    );
    assert_eq!(
        decode_hex(vector_hex("corrupt_header")).unwrap(),
        corrupt_header()
    );
}

#[test]
#[ignore = "only run manually to regenerate test vectors"]
fn regenerate_vectors() {
    for (name, bytes) in [
        ("valid_c1", valid_c1()),
        ("valid_empty", valid_empty()),
        ("corrupt_header", corrupt_header()),
    ] {
        println!("{name}:{}", encode_hex(&bytes));
    }
}

fn vector_hex(name: &str) -> &'static str {
    match name {
        "valid_c1" => include_str!("vectors/valid_c1.qzt.hex"),
        "valid_empty" => include_str!("vectors/valid_empty.qzt.hex"),
        "corrupt_header" => include_str!("vectors/corrupt_header.qzt.hex"),
        _ => panic!("unknown vector {name}"),
    }
    .trim()
}

fn valid_c1() -> Vec<u8> {
    pack_bytes(b"alpha\nbeta\n", WriterOptions::default()).expect("pack valid_c1")
}

fn valid_empty() -> Vec<u8> {
    pack_bytes(b"", WriterOptions::default()).expect("pack valid_empty")
}

fn corrupt_header() -> Vec<u8> {
    let mut bytes = valid_c1();
    bytes[0] ^= 0xff;
    bytes
}

fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    let clean = input.trim();
    if !clean.len().is_multiple_of(2) {
        return Err("odd hex length".to_owned());
    }
    let mut bytes = Vec::with_capacity(clean.len() / 2);
    for index in (0..clean.len()).step_by(2) {
        let value =
            u8::from_str_radix(&clean[index..index + 2], 16).map_err(|error| error.to_string())?;
        bytes.push(value);
    }
    Ok(bytes)
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("writing to String never fails");
    }
    output
}
