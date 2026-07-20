use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

use qzt::error::QztError;
use qzt::fixed::{FooterTrailer, Header};
use qzt::format::{FOOTER_TRAILER_LEN, HEADER_LEN};
use qzt::reader::{QztReader, VerifyLevel};
use qzt::schema::{Checksum, FooterPayload};
use qzt::skeleton::open_skeleton_details;
use qzt::writer::{DocumentSpan, WriterBuilder, WriterOptions, pack_bytes};
mod support;
use support::writer_options;

const REQUIRED_VECTORS: [&str; 14] = [
    "valid_c1",
    "valid_empty",
    "valid_crlf",
    "valid_mixed_newline",
    "valid_utf8_multibyte",
    "valid_multi_chunk",
    "valid_no_trailing_newline",
    "valid_dense_line_index",
    "valid_document_index",
    "corrupt_header",
    "corrupt_footer_checksum",
    "corrupt_chunk_data",
    "corrupt_truncated",
    "corrupt_noncanonical_cbor",
];

// The UTF-8 fixture deliberately contains `e` plus a combining accent. Keeping
// the non-NFC sequence proves that readers preserve bytes without normalization.
#[allow(clippy::unicode_not_nfc)]
const FROZEN_MANIFEST_ROWS: [&str; 14] = [
    "valid_c1\thex\tok\tok\talpha\\nbeta\\n\t-",
    "valid_empty\thex\tok\tok\t\t-",
    "valid_crlf\thex\tok\tok\ta\\r\\nb\\r\\n\t-",
    "valid_mixed_newline\thex\tok\tok\ta\\nb\\r\\n\t-",
    "valid_utf8_multibyte\thex\tok\tok\t日本語🙂é\\n\t-",
    "valid_multi_chunk\thex\tok\tok\talpha\\nbeta\\ngamma\\n\t-",
    "valid_no_trailing_newline\thex\tok\tok\ta\\nb\t-",
    "valid_dense_line_index\thex\tok\tok\tzero\\none\\ntwo\\n\t-",
    "valid_document_index\thex\tok\tok\tdocument one\\n\t-",
    "corrupt_header\thex\terr\t-\t-\tinvalid_magic",
    "corrupt_footer_checksum\thex\terr\t-\t-\tfooter_checksum_mismatch",
    "corrupt_chunk_data\thex\tok\terr\t-\tcompressed_chunk_checksum_mismatch",
    "corrupt_truncated\thex\terr\t-\t-\tinvalid_footer_trailer",
    "corrupt_noncanonical_cbor\thex\terr\t-\t-\tnon_canonical_cbor",
];

const FROZEN_EXTENSION_ROWS: [&str; 2] = [
    "valid_dense_line_index\ttrue\tfalse\t-\t-\t-\t-\t-\t-\t-\t-",
    "valid_document_index\tfalse\ttrue\tdoc-1\t0\t13\t0\t1\t0\t1\tblake3:2ec48cafde4afeeeffcb2264d1b080d3f91b542682fd246ce57fdc926c64ece9",
];

const FROZEN_VECTOR_BLAKE3: [(&str, &str); 14] = [
    (
        "valid_c1",
        "ba2af047411719ce4c439d6c8d6171bf5d50506f2de7b3fa28e69c5c239a3d03",
    ),
    (
        "valid_empty",
        "5d3ef0d2bdf977c9b48046856437c23e49e698e0a8383d89402ced8b93ef884b",
    ),
    (
        "valid_crlf",
        "6ad783acb3406a406f72a752d35aef0a58820013597a323b5333d061b9306c1a",
    ),
    (
        "valid_mixed_newline",
        "007038e125559a646c68b1d9787df69eeb05f8669d1224ac4649efc0c9c249cb",
    ),
    (
        "valid_utf8_multibyte",
        "9d5eadbcba84bbbe1f3a0200c0e2b0a08854d55a34b1e1c54c56e974d313c4f3",
    ),
    (
        "valid_multi_chunk",
        "b1c39822af64e77f69144646b32b966e35e66345a5a03a190cb359a9fed44e7c",
    ),
    (
        "valid_no_trailing_newline",
        "9a65476bdabbbb441a3617b86082ef2e51731a67cccccb1f8918fd597fa3d4c0",
    ),
    (
        "valid_dense_line_index",
        "bfd94258110aea44e879096a45e499c1c79f3124a2ac2dd2956af63ee13f16a9",
    ),
    (
        "valid_document_index",
        "9dbb05d5e42a308a995a59b1a1c389bdf71332a03f06996421817c3787275605",
    ),
    (
        "corrupt_header",
        "833c28f5f8c9f65efb81199b674f8e15582cb7c5dd9afef579cb1e9176207ada",
    ),
    (
        "corrupt_footer_checksum",
        "81258d3a122be82d660c640772ebd629aed5a0e62c0ff230b623cbd3217a0906",
    ),
    (
        "corrupt_chunk_data",
        "36d2ff056129d68954d9b1e4e4cee98ed513e24873c3d01fbb62b270c9f2e77c",
    ),
    (
        "corrupt_truncated",
        "29e3828faa7f44540d03bbbc23d287ae40ab4d52fd4105b167a231fed27db37c",
    ),
    (
        "corrupt_noncanonical_cbor",
        "9c90260a999b6e1080e31f962e9e808a336d7831419a23f706401dfae269a9b1",
    ),
];

#[test]
fn published_manifest_has_v1_schema_and_required_coverage() {
    let manifest = include_str!("vectors/manifest.tsv");
    let mut lines = manifest.lines();
    assert_eq!(
        lines.next(),
        Some("name\tkind\texpect_open\texpect_deep_verify\texpect_export_text\texpect_error")
    );

    let rows = lines.collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        FROZEN_MANIFEST_ROWS.len(),
        "every published manifest row must be added to the frozen registry"
    );
    assert_eq!(
        rows.as_slice(),
        FROZEN_MANIFEST_ROWS.as_slice(),
        "published manifest expectations are immutable"
    );
    let names = rows
        .iter()
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(
                fields.len(),
                6,
                "manifest row must have six columns: {line}"
            );
            match (fields[2], fields[3], fields[4], fields[5]) {
                ("ok", "ok", export, "-") if export != "-" => {}
                ("ok", "err", "-", error) if error != "-" => {}
                ("err", "-", "-", error) if error != "-" => {}
                _ => panic!("invalid expectation state for manifest row: {line}"),
            }
            fields[0]
        })
        .collect::<Vec<_>>();
    assert!(
        names.len() >= 14,
        "vector set v1 requires at least 14 cases"
    );
    for required in REQUIRED_VECTORS {
        assert!(
            names.contains(&required),
            "missing required vector {required}"
        );
    }

    let mut unique_names = names.clone();
    unique_names.sort_unstable();
    unique_names.dedup();
    assert_eq!(
        unique_names.len(),
        names.len(),
        "vector names must be unique"
    );

    let frozen_names = frozen_vector_names();
    let mut manifest_names = names.clone();
    manifest_names.sort_unstable();
    assert_eq!(
        manifest_names, frozen_names,
        "every manifest vector must have exactly one frozen hash"
    );

    let mut file_names = fs::read_dir(vector_dir())
        .expect("read vector directory")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".qzt.hex").map(str::to_owned)
        })
        .collect::<Vec<_>>();
    file_names.sort_unstable();
    assert_eq!(
        file_names, frozen_names,
        "committed vector files and frozen hashes must be the same set"
    );
}

#[test]
fn portable_vector_runner_matches_manifest() {
    let manifest = include_str!("vectors/manifest.tsv");
    for line in manifest.lines().skip(1) {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(
            fields.len(),
            6,
            "manifest row must have six columns: {line}"
        );
        let [
            name,
            kind,
            expect_open,
            expect_deep,
            expect_export,
            expect_error,
        ] = fields.as_slice()
        else {
            unreachable!("column count was checked above")
        };
        assert_eq!(*kind, "hex", "unsupported vector kind for {name}");
        let bytes = decode_hex(&vector_hex(name)).expect("vector hex should decode");

        match *expect_open {
            "ok" => {
                let reader = QztReader::open(&bytes).expect("vector should open");
                match *expect_deep {
                    "ok" => {
                        reader
                            .verify(VerifyLevel::Deep)
                            .expect("deep verify should pass");
                    }
                    "err" => assert_error_category(
                        reader
                            .verify(VerifyLevel::Deep)
                            .expect_err("deep verify should fail"),
                        expect_error,
                        name,
                    ),
                    other => panic!("unknown deep-verify expectation {other}"),
                }
                if *expect_export != "-" {
                    assert_eq!(
                        reader.export_all().expect("export should pass"),
                        unescape_manifest_text(expect_export)
                            .expect("manifest export text uses the documented escape grammar")
                            .as_bytes(),
                        "export mismatch for {name}"
                    );
                }
            }
            "err" => match QztReader::open(&bytes) {
                Ok(_) => panic!("vector open should fail for {name}"),
                Err(error) => assert_error_category(error, expect_error, name),
            },
            other => panic!("unknown open expectation {other}"),
        }
    }
}

#[test]
fn vectors_regenerate_byte_identically() {
    for (name, generated) in generated_vectors() {
        assert_eq!(
            decode_hex(&vector_hex(name)).expect("committed vector should decode"),
            generated,
            "generated bytes changed for frozen vector {name}"
        );
    }
}

#[test]
fn valid_vectors_encode_the_declared_core_features() {
    let crlf = open_skeleton_details(&valid_crlf()).expect("CRLF vector opens");
    assert_eq!(crlf.metadata.newline_mode, "crlf");

    let mixed = open_skeleton_details(&valid_mixed_newline()).expect("mixed vector opens");
    assert_eq!(mixed.metadata.newline_mode, "mixed");

    let multi = open_skeleton_details(&valid_multi_chunk()).expect("multi-chunk vector opens");
    assert!(multi.chunk_entries.len() > 1);

    let dense = open_skeleton_details(&valid_dense_line_index()).expect("dense vector opens");
    assert!(dense.metadata.dense_line_index);
    assert!(dense.dense_line_index.is_some());

    let document = open_skeleton_details(&valid_document_index()).expect("document vector opens");
    assert!(document.metadata.document_index);
    assert_eq!(
        document
            .document_index
            .expect("Document Index block exists")
            .documents
            .len(),
        1
    );
}

#[test]
fn extension_aware_runner_matches_index_expectations() {
    let manifest = include_str!("vectors/extensions.tsv");
    let mut lines = manifest.lines();
    assert_eq!(
        lines.next(),
        Some(
            "name\texpect_dense_line_index\texpect_document_index\tdoc_id\tlogical_offset\tbyte_length\tfirst_line\tline_count\tchunk_start\tchunk_end\tchecksum"
        )
    );

    let rows = lines.collect::<Vec<_>>();
    assert_eq!(
        rows.as_slice(),
        FROZEN_EXTENSION_ROWS.as_slice(),
        "published extension expectations are immutable"
    );

    for line in rows {
        let fields = line.split('\t').collect::<Vec<_>>();
        let [
            name,
            expect_dense,
            expect_document,
            doc_id,
            logical_offset,
            byte_length,
            first_line,
            line_count,
            chunk_start,
            chunk_end,
            checksum,
        ] = fields.as_slice()
        else {
            panic!("extension row must have eleven columns: {line}")
        };
        let bytes = decode_hex(&vector_hex(name)).expect("extension vector hex should decode");
        let details = open_skeleton_details(&bytes).expect("extension vector should open");
        assert_eq!(details.dense_line_index.is_some(), *expect_dense == "true");
        assert_eq!(details.document_index.is_some(), *expect_document == "true");

        if *expect_dense == "true" {
            let dense = details
                .dense_line_index
                .as_ref()
                .expect("expected Dense Line Index");
            assert_eq!(dense.entries.len(), 1);
            assert_eq!(dense.entries[0].line_start_offsets, [0, 5, 9]);
            let reader = QztReader::open(&bytes).expect("dense vector reader opens");
            assert_eq!(reader.read_line_raw(0).unwrap(), b"zero\n");
            assert_eq!(reader.read_line_raw(1).unwrap(), b"one\n");
            assert_eq!(reader.read_line_raw(2).unwrap(), b"two\n");
        }

        if *expect_document == "true" {
            let documents = details
                .document_index
                .as_ref()
                .expect("expected Document Index")
                .documents
                .as_slice();
            assert_eq!(documents.len(), 1);
            let document = &documents[0];
            assert_eq!(document.doc_id, *doc_id);
            assert_eq!(document.logical_offset, parse_u64(logical_offset));
            assert_eq!(document.byte_length, parse_u64(byte_length));
            assert_eq!(document.first_line, parse_u64(first_line));
            assert_eq!(document.line_count, parse_u64(line_count));
            assert_eq!(document.chunk_start, parse_u64(chunk_start));
            assert_eq!(document.chunk_end, parse_u64(chunk_end));
            assert_eq!(
                format!(
                    "{}:{}",
                    document.checksum.algorithm,
                    encode_hex(&document.checksum.value)
                ),
                *checksum
            );
            let reader = QztReader::open(&bytes).expect("document vector reader opens");
            assert_eq!(
                reader
                    .read_document_verified(doc_id, &document.checksum)
                    .expect("document lookup and checksum verification succeed"),
                b"document one\n"
            );
        } else {
            assert!(fields[3..].iter().all(|value| *value == "-"));
        }
    }
}

fn parse_u64(value: &str) -> u64 {
    value.parse().expect("extension expectation must be u64")
}

#[test]
fn published_vector_files_match_frozen_blake3() {
    assert_eq!(
        FROZEN_VECTOR_BLAKE3.len(),
        REQUIRED_VECTORS.len(),
        "every published vector must have a frozen hash"
    );
    let frozen_names = frozen_vector_names();
    let mut required_names = REQUIRED_VECTORS.to_vec();
    required_names.sort_unstable();
    assert_eq!(
        frozen_names, required_names,
        "frozen hashes must cover exactly the published vectors"
    );
    for (name, expected) in FROZEN_VECTOR_BLAKE3 {
        let path = vector_dir().join(format!("{name}.qzt.hex"));
        let file = fs::read(path).expect("read frozen vector file");
        let actual = blake3::hash(&file);
        assert_eq!(
            actual.to_hex().as_str(),
            expected,
            "hash changed for {name}"
        );
    }
}

fn frozen_vector_names() -> Vec<&'static str> {
    let mut names = FROZEN_VECTOR_BLAKE3
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>();
    names.sort_unstable();
    names
}

#[test]
#[ignore = "only run manually to regenerate test vectors"]
fn regenerate_vectors() {
    let candidate_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/conformance-vectors-candidate");
    fs::create_dir_all(&candidate_dir).expect("create candidate vector directory");
    assert!(
        !fs::symlink_metadata(&candidate_dir)
            .expect("inspect candidate vector directory")
            .file_type()
            .is_symlink(),
        "candidate directory must not be a symlink"
    );
    for (name, bytes) in generated_vectors() {
        let path = candidate_dir.join(format!("{name}.qzt.hex"));
        let contents = format!("{}\n", encode_hex(&bytes));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => file
                .write_all(contents.as_bytes())
                .expect("write complete candidate vector"),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                assert_eq!(
                    fs::read(&path).expect("read existing candidate vector"),
                    contents.as_bytes(),
                    "candidate {name} already exists with different bytes; remove the candidate directory explicitly before regenerating"
                );
            }
            Err(error) => panic!("create candidate vector {name}: {error}"),
        }
        println!("{name}\t{}", blake3::hash(contents.as_bytes()).to_hex());
    }
}

fn generated_vectors() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("valid_c1", valid_c1()),
        ("valid_empty", valid_empty()),
        ("valid_crlf", valid_crlf()),
        ("valid_mixed_newline", valid_mixed_newline()),
        ("valid_utf8_multibyte", valid_utf8_multibyte()),
        ("valid_multi_chunk", valid_multi_chunk()),
        ("valid_no_trailing_newline", valid_no_trailing_newline()),
        ("valid_dense_line_index", valid_dense_line_index()),
        ("valid_document_index", valid_document_index()),
        ("corrupt_header", corrupt_header()),
        ("corrupt_footer_checksum", corrupt_footer_checksum()),
        ("corrupt_chunk_data", corrupt_chunk_data()),
        ("corrupt_truncated", corrupt_truncated()),
        ("corrupt_noncanonical_cbor", corrupt_noncanonical_cbor()),
    ]
}

fn vector_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors")
}

fn vector_hex(name: &str) -> String {
    fs::read_to_string(vector_dir().join(format!("{name}.qzt.hex")))
        .unwrap_or_else(|error| panic!("read vector {name}: {error}"))
}

fn valid_c1() -> Vec<u8> {
    pack_bytes(b"alpha\nbeta\n", WriterOptions::default()).expect("pack valid_c1")
}

fn valid_empty() -> Vec<u8> {
    pack_bytes(b"", WriterOptions::default()).expect("pack valid_empty")
}

fn valid_crlf() -> Vec<u8> {
    pack_bytes(b"a\r\nb\r\n", WriterOptions::default()).expect("pack CRLF vector")
}

fn valid_mixed_newline() -> Vec<u8> {
    pack_bytes(b"a\nb\r\n", WriterOptions::default()).expect("pack mixed-newline vector")
}

fn valid_utf8_multibyte() -> Vec<u8> {
    pack_bytes("日本語🙂e\u{301}\n".as_bytes(), WriterOptions::default())
        .expect("pack multibyte UTF-8 vector")
}

fn valid_multi_chunk() -> Vec<u8> {
    pack_bytes(b"alpha\nbeta\ngamma\n", writer_options(8, 8)).expect("pack multi-chunk vector")
}

fn valid_no_trailing_newline() -> Vec<u8> {
    pack_bytes(b"a\nb", WriterOptions::default()).expect("pack no-trailing-newline vector")
}

fn valid_dense_line_index() -> Vec<u8> {
    WriterBuilder::new()
        .dense_line_index(true)
        .pack(b"zero\none\ntwo\n")
        .expect("pack Dense Line Index vector")
}

fn valid_document_index() -> Vec<u8> {
    let input = b"document one\n";
    WriterBuilder::new()
        .document_spans(vec![DocumentSpan::new(
            "doc-1",
            0,
            u64::try_from(input.len()).expect("fixture length fits u64"),
        )])
        .pack(input)
        .expect("pack Document Index vector")
}

fn corrupt_header() -> Vec<u8> {
    let mut bytes = valid_c1();
    bytes[0] ^= 0xff;
    bytes
}

fn corrupt_footer_checksum() -> Vec<u8> {
    let mut bytes = valid_c1();
    // The final 32 trailer bytes are the footer-payload BLAKE3 checksum.
    let checksum_byte = bytes.len() - 1;
    bytes[checksum_byte] ^= 0x01;
    bytes
}

fn corrupt_chunk_data() -> Vec<u8> {
    let mut bytes = valid_c1();
    // The first compressed frame starts immediately after the fixed header.
    // Open remains structural; deep verification detects its stored checksum.
    bytes[HEADER_LEN] ^= 0x01;
    bytes
}

fn corrupt_truncated() -> Vec<u8> {
    let mut bytes = valid_c1();
    // Remove half of the fixed footer trailer, including checksum bytes.
    bytes.truncate(bytes.len() - 32);
    bytes
}

fn corrupt_noncanonical_cbor() -> Vec<u8> {
    let mut bytes = valid_c1();
    let header = Header::decode(&bytes[..HEADER_LEN]).expect("decode fixture header");
    let metadata_start = usize::try_from(header.metadata_offset).expect("metadata offset fits");
    let metadata_size = usize::try_from(header.metadata_size).expect("metadata size fits");
    let metadata_end = metadata_start + metadata_size;

    let format_pair = b"\x66format\x63qzt";
    let schema_pair = b"\x66schema\x6fqzt.metadata.v1";
    let metadata = bytes[metadata_start..metadata_end].to_vec();
    let format_start = find_subslice(&metadata, format_pair).expect("format pair exists");
    let schema_start = find_subslice(&metadata, schema_pair).expect("schema pair exists");
    assert_eq!(format_start + format_pair.len(), schema_start);

    // Swapping same-length-key map entries preserves valid CBOR semantics and
    // total block size, but violates deterministic canonical key ordering.
    let mut noncanonical = Vec::with_capacity(metadata.len());
    noncanonical.extend_from_slice(&metadata[..format_start]);
    noncanonical.extend_from_slice(schema_pair);
    noncanonical.extend_from_slice(format_pair);
    noncanonical.extend_from_slice(&metadata[schema_start + schema_pair.len()..]);
    assert_eq!(noncanonical.len(), metadata.len());
    bytes[metadata_start..metadata_end].copy_from_slice(&noncanonical);

    let trailer_start = bytes.len() - FOOTER_TRAILER_LEN;
    let mut trailer =
        FooterTrailer::decode(&bytes[trailer_start..]).expect("decode fixture trailer");
    let footer_start = usize::try_from(trailer.footer_payload_offset).expect("footer offset fits");
    let footer_end =
        footer_start + usize::try_from(trailer.footer_payload_size).expect("footer size fits");
    let mut footer = FooterPayload::decode(&bytes[footer_start..footer_end])
        .expect("decode fixture footer payload");
    footer.metadata.checksum = Checksum::blake3(&noncanonical);
    if footer.container_checksum.is_some() {
        footer.container_checksum = Some(Checksum::blake3(&bytes[..footer_start]));
    }
    let footer_bytes = footer.encode().expect("re-encode footer payload");
    assert_eq!(footer_bytes.len(), footer_end - footer_start);
    bytes[footer_start..footer_end].copy_from_slice(&footer_bytes);
    trailer.footer_payload_checksum_blake3 = Checksum::blake3(&footer_bytes).value;
    bytes[trailer_start..].copy_from_slice(&trailer.encode());
    bytes
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn assert_error_category(error: QztError, expected: &str, name: &str) {
    assert_eq!(
        error_category(error),
        expected,
        "unexpected error category for {name}: {error}"
    );
}

fn error_category(error: QztError) -> &'static str {
    match error {
        QztError::InvalidMagic => "invalid_magic",
        QztError::InvalidFooterTrailer => "invalid_footer_trailer",
        QztError::FooterChecksumMismatch => "footer_checksum_mismatch",
        QztError::CompressedChunkChecksumMismatch => "compressed_chunk_checksum_mismatch",
        QztError::NonCanonicalCbor => "non_canonical_cbor",
        _ => "unexpected_error_category",
    }
}

#[test]
fn manifest_export_escape_grammar_is_strict_and_byte_preserving() {
    assert_eq!(
        unescape_manifest_text("slash=\\\\ tab=\\t cr=\\r lf=\\n 日本語").unwrap(),
        "slash=\\ tab=\t cr=\r lf=\n 日本語"
    );
    assert!(unescape_manifest_text("unknown=\\x").is_err());
    assert!(unescape_manifest_text("dangling=\\").is_err());
}

fn unescape_manifest_text(input: &str) -> Result<String, String> {
    let mut output = String::with_capacity(input.len());
    let mut characters = input.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let escaped = characters
            .next()
            .ok_or_else(|| "dangling manifest escape".to_owned())?;
        output.push(match escaped {
            '\\' => '\\',
            't' => '\t',
            'r' => '\r',
            'n' => '\n',
            other => return Err(format!("unknown manifest escape \\{other}")),
        });
    }
    Ok(output)
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
