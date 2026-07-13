use std::fmt::Write as _;
use std::fs;
use std::process::Command;

use qzt::{
    Checksum, NgramIndexBuildOptions, QziFileSidecar, QziSidecar, QztError, QztFileReader,
    QztReader, RawNgramIndex, SearchOptions, SidecarIndexKind, VerifyLevel, build_search_sidecar,
    pack_bytes_with_container_id,
};
mod support;
use support::{
    CountingReadAt, assert_semantic_report_eq, assert_success, output_success, writer_options,
};

#[test]
fn wrong_sidecar_schema_is_rejected() {
    let (container, mut sidecar) = token_sidecar_fixture(b"alpha\n", [0xf2; 16]);
    patch_sidecar_schema(&mut sidecar);
    assert_sidecar_open_errors(
        &container,
        &sidecar,
        QztError::ContainerCorrupt,
        "schema mismatch",
    );
    assert_core_deep_verify_ok(&container);
}

#[test]
fn wrong_source_container_id_sidecar_is_rejected() {
    let (container, mut sidecar) = token_sidecar_fixture(b"alpha\n", [0xe0; 16]);
    replace_first(&mut sidecar, &[0xe0; 16], &[0xee; 16]);
    assert_sidecar_open_errors(
        &container,
        &sidecar,
        QztError::ContainerIdMismatch,
        "container id mismatch",
    );
    assert_core_deep_verify_ok(&container);
}

#[test]
fn wrong_source_original_checksum_sidecar_is_rejected() {
    let input = b"alpha\n";
    let (container, mut sidecar) = token_sidecar_fixture(input, [0xe1; 16]);
    let checksum = Checksum::blake3(input).value;
    let mut wrong = checksum;
    wrong[0] ^= 0xff;
    replace_first(&mut sidecar, &checksum, &wrong);
    assert_sidecar_open_errors(
        &container,
        &sidecar,
        QztError::ContainerCorrupt,
        "original checksum mismatch",
    );
    assert_core_deep_verify_ok(&container);
}

#[test]
fn wrong_source_qzt_footer_checksum_sidecar_is_rejected() {
    let (container, mut sidecar) = token_sidecar_fixture(b"alpha\n", [0xf3; 16]);
    patch_sidecar_source_qzt_footer_checksum(&mut sidecar);
    assert_sidecar_open_errors(
        &container,
        &sidecar,
        QztError::ContainerCorrupt,
        "footer checksum mismatch",
    );
    assert_core_deep_verify_ok(&container);
}

#[test]
fn missing_or_rejected_sidecar_does_not_break_core_operations() {
    let input = b"alpha\nbeta\n";
    let container = pack_bytes_with_container_id(input, [0xe2; 16], writer_options(8, 8))
        .expect("container should pack");
    let reader = QztReader::open(&container).expect("reader should open without sidecar");

    assert_eq!(reader.export_all().expect("export should work"), input);
    assert!(reader.verify(VerifyLevel::Deep).is_ok());
}

#[test]
fn sidecar_lookup_matches_transient_ngram_index_behavior() {
    let input = "東京大学\n京都大学\n";
    let container =
        pack_bytes_with_container_id(input.as_bytes(), [0xe3; 16], writer_options(8, 8))
            .expect("container should pack");
    let sidecar = build_search_sidecar(&container, SidecarIndexKind::Ngram { n: 2 })
        .expect("sidecar should build");
    let sidecar = QziSidecar::open(&container, &sidecar).expect("sidecar should open");
    let reader = QztReader::open(&container).expect("reader should open");
    let transient = RawNgramIndex::build_from_container(
        &container,
        NgramIndexBuildOptions {
            n: 2,
            ..NgramIndexBuildOptions::default()
        },
    )
    .expect("transient index should build");

    let sidecar_report = sidecar
        .search(&reader, "東京", SearchOptions::default())
        .expect("sidecar search should run");
    let transient_report = transient
        .search(&reader, "東京", SearchOptions::default())
        .expect("transient search should run");

    assert_semantic_report_eq(&sidecar_report, &transient_report, "sidecar vs transient");
}

#[test]
fn common_term_sidecar_query_is_capped_without_decoding_candidates() {
    let mut input = String::new();
    for index in 0..128 {
        let _ = writeln!(input, "aaa common {index}");
    }
    let container =
        pack_bytes_with_container_id(input.as_bytes(), [0xe4; 16], writer_options(256, 256))
            .expect("container should pack");
    let sidecar = build_search_sidecar(&container, SidecarIndexKind::Ngram { n: 3 })
        .expect("sidecar should build");
    let sidecar = QziSidecar::open(&container, &sidecar).expect("sidecar should open");
    let reader = QztReader::open(&container).expect("reader should open");

    let report = sidecar
        .search(
            &reader,
            "aaa",
            SearchOptions {
                max_candidate_granules: 10,
                ..SearchOptions::default()
            },
        )
        .expect("sidecar search should run");

    assert!(report.capped);
    assert_eq!(report.metrics.decoded_bytes, 0);
}

#[test]
fn rare_term_sidecar_query_decodes_only_candidate_overlapping_chunks() {
    let mut input = String::new();
    for index in 0..64 {
        let _ = writeln!(input, "info line {index}");
    }
    input.push_str("zzztarget\n");
    let container =
        pack_bytes_with_container_id(input.as_bytes(), [0xe5; 16], writer_options(64, 64))
            .expect("container should pack");
    let sidecar = build_search_sidecar(&container, SidecarIndexKind::Ngram { n: 3 })
        .expect("sidecar should build");
    let sidecar = QziSidecar::open(&container, &sidecar).expect("sidecar should open");
    let reader = QztReader::open(&container).expect("reader should open");

    let report = sidecar
        .search(&reader, "zzz", SearchOptions::default())
        .expect("sidecar search should run");

    assert_eq!(report.metrics.verified_matches, 1);
    assert!(report.metrics.candidate_chunks < reader.info().chunk_count);
    assert!(report.metrics.decoded_bytes < reader.info().original_size);
}

#[test]
fn cli_rebuilds_sidecar_and_searches_with_it() {
    let base = std::env::temp_dir().join(format!("qzt-phase13-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    let sidecar = base.join("input.qzt.qzi");
    fs::write(&input, "東京大学\n京都大学\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );
    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("sidecar-rebuild")
            .arg(&packed)
            .arg("-o")
            .arg(&sidecar)
            .arg("--index")
            .arg("ngram")
            .arg("--ngram")
            .arg("2"),
    );
    let output = output_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("search")
            .arg(&packed)
            .arg("東京")
            .arg("--sidecar")
            .arg(&sidecar),
    );
    let output = String::from_utf8(output).expect("output should be utf-8");

    assert!(output.contains("source=verified_original_bytes"));
    assert!(output.contains("index_kind=ngram"));

    let _ = fs::remove_dir_all(base);
}

#[test]
fn unsupported_source_format_version_sidecar_is_rejected() {
    let input = b"alpha\n";
    let container = pack_bytes_with_container_id(input, [0xec; 16], writer_options(64, 64))
        .expect("container should pack");
    let sidecar =
        build_search_sidecar(&container, SidecarIndexKind::Token).expect("sidecar should build");

    for (major, minor) in [(0, 2), (1, 0)] {
        let mut patched = sidecar.clone();
        patch_sidecar_source_format_version(&mut patched, major, minor);
        assert_sidecar_open_errors(
            &container,
            &patched,
            QztError::UnsupportedVersion,
            &format!("unsupported source format version [{major}, {minor}]"),
        );
    }
    assert_core_deep_verify_ok(&container);
}

#[test]
fn rejected_sidecar_does_not_break_core_operations() {
    let input = b"alpha\nbeta\n";
    let container = pack_bytes_with_container_id(input, [0xed; 16], writer_options(8, 8))
        .expect("container should pack");
    let mut sidecar =
        build_search_sidecar(&container, SidecarIndexKind::Token).expect("sidecar should build");
    patch_sidecar_source_format_version(&mut sidecar, 1, 0);

    assert_eq!(
        QziSidecar::open(&container, &sidecar).map(|_| ()),
        Err(QztError::UnsupportedVersion)
    );

    let reader = QztReader::open(&container).expect("reader should open without sidecar");
    assert_eq!(reader.export_all().expect("export should work"), input);
    assert!(reader.verify(VerifyLevel::Deep).is_ok());
}

#[test]
fn cli_search_reports_user_readable_unsupported_sidecar_version() {
    let base = std::env::temp_dir().join(format!("qzt-phase13-version-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    let sidecar_path = base.join("input.qzt.qzi");
    fs::write(&input, "東京大学\n京都大学\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );
    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("sidecar-rebuild")
            .arg(&packed)
            .arg("-o")
            .arg(&sidecar_path)
            .arg("--index")
            .arg("token"),
    );

    let mut sidecar = fs::read(&sidecar_path).expect("sidecar should be readable");
    patch_sidecar_source_format_version(&mut sidecar, 0, 2);
    fs::write(&sidecar_path, &sidecar).expect("patched sidecar should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("search")
        .arg(&packed)
        .arg("東京")
        .arg("--sidecar")
        .arg(&sidecar_path)
        .output()
        .expect("search command should run");

    assert_eq!(output.status.code(), Some(1), "search must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported QZT format version"),
        "stderr must use user-facing version message: {stderr}"
    );
    assert!(
        !stderr.contains("UnsupportedVersion"),
        "stderr must not expose internal error names: {stderr}"
    );

    let _ = fs::remove_dir_all(base);
}

#[test]
fn truncated_section_sidecar_is_rejected() {
    let (container, mut sidecar) = token_sidecar_fixture(b"alpha\n", [0xee; 16]);
    sidecar.pop();
    assert_sidecar_open_errors(
        &container,
        &sidecar,
        QztError::UnexpectedEof,
        "truncated section",
    );
    assert_core_deep_verify_ok(&container);
}

#[test]
fn section_checksum_bit_flip_sidecar_is_rejected() {
    let (container, mut sidecar) = token_sidecar_fixture(b"alpha\n", [0xef; 16]);
    flip_first_sidecar_payload_byte(&mut sidecar);
    assert_sidecar_open_errors(
        &container,
        &sidecar,
        QztError::ContainerCorrupt,
        "checksum bit flip",
    );
    assert_core_deep_verify_ok(&container);
}

#[test]
fn non_zero_term_flags_sidecar_is_rejected() {
    let input = b"alpha\n";
    let (container, mut sidecar) = token_sidecar_fixture(input, [0xf1; 16]);
    patch_sidecar_first_term_flags(&mut sidecar, 1);
    assert_sidecar_open_errors(
        &container,
        &sidecar,
        QztError::InvalidFlags,
        "non-zero term flags",
    );

    let reader = QztReader::open(&container).expect("reader should open");
    assert_eq!(
        reader.export_all().expect("export should work"),
        input.as_slice()
    );
    reader
        .verify(VerifyLevel::Deep)
        .expect("core deep verify should succeed after sidecar rejection");
}

#[test]
fn zero_length_granules_section_sidecar_is_rejected() {
    let (container, mut sidecar) = token_sidecar_fixture(b"", [0xf0; 16]);
    patch_sidecar_granules_size_to_zero(&mut sidecar);
    assert_zero_length_granules_section_rejected(&container, &sidecar);
    assert_core_deep_verify_ok(&container);
}

#[test]
fn truncated_section_sidecar_cli_exits_without_panic() {
    run_corrupted_sidecar_cli_test("truncated", |sidecar| {
        sidecar.pop();
    });
}

#[test]
fn section_checksum_bit_flip_sidecar_cli_exits_without_panic() {
    run_corrupted_sidecar_cli_test("checksum-bit-flip", |sidecar| {
        flip_first_sidecar_payload_byte(sidecar);
    });
}

#[test]
fn zero_length_granules_section_sidecar_cli_exits_without_panic() {
    let base = std::env::temp_dir().join(format!(
        "qzt-phase13-corrupt-zero-length-granules-{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    let sidecar_path = base.join("input.qzt.qzi");
    fs::write(&input, b"").expect("empty input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );
    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("sidecar-rebuild")
            .arg(&packed)
            .arg("-o")
            .arg(&sidecar_path)
            .arg("--index")
            .arg("token"),
    );

    let mut sidecar = fs::read(&sidecar_path).expect("sidecar should be readable");
    patch_sidecar_granules_size_to_zero(&mut sidecar);
    fs::write(&sidecar_path, &sidecar).expect("corrupted sidecar should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("search")
        .arg(&packed)
        .arg("query")
        .arg("--sidecar")
        .arg(&sidecar_path)
        .output()
        .expect("search command should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "search must fail on zero-length granules sidecar"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.is_empty(),
        "stderr must contain a user-facing error"
    );
    assert!(
        !stderr.contains("panicked"),
        "zero-length granules sidecar must not panic: {stderr}"
    );

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("verify")
            .arg(&packed)
            .arg("--deep"),
    );

    let _ = fs::remove_dir_all(base);
}

fn token_sidecar_fixture(input: &[u8], container_id: [u8; 16]) -> (Vec<u8>, Vec<u8>) {
    let container = pack_bytes_with_container_id(input, container_id, writer_options(64, 64))
        .expect("container should pack");
    let sidecar =
        build_search_sidecar(&container, SidecarIndexKind::Token).expect("sidecar should build");
    (container, sidecar)
}

fn assert_core_deep_verify_ok(container: &[u8]) {
    let reader = QztReader::open(container).expect("reader should open");
    reader
        .verify(VerifyLevel::Deep)
        .expect("core deep verify should succeed after sidecar rejection");
}

fn assert_sidecar_open_errors(container: &[u8], sidecar: &[u8], expected: QztError, label: &str) {
    assert_eq!(
        QziSidecar::open(container, sidecar).map(|_| ()),
        Err(expected),
        "in-memory open should reject {label}"
    );
    let file_reader = QztFileReader::open_read_at(container, container.len() as u64)
        .expect("file reader should open");
    assert_eq!(
        QziFileSidecar::open_read_at(sidecar, sidecar.len() as u64, &file_reader).map(|_| ()),
        Err(expected),
        "file sidecar open should reject {label}"
    );
}

fn assert_zero_length_granules_section_rejected(container: &[u8], sidecar: &[u8]) {
    let memory_error = QziSidecar::open(container, sidecar).expect_err("in-memory open");
    assert!(
        matches!(
            memory_error,
            QztError::UnexpectedEof | QztError::ContainerCorrupt
        ),
        "in-memory open should reject zero-length granules section: {memory_error:?}"
    );

    let file_reader = QztFileReader::open_read_at(container, container.len() as u64)
        .expect("file reader should open");
    assert_eq!(
        QziFileSidecar::open_read_at(sidecar, sidecar.len() as u64, &file_reader).map(|_| ()),
        Err(QztError::ContainerCorrupt),
        "file sidecar open should reject zero-length granules section"
    );
}

fn run_corrupted_sidecar_cli_test(label: &str, corrupt: impl FnOnce(&mut Vec<u8>)) {
    let base = std::env::temp_dir().join(format!(
        "qzt-phase13-corrupt-{label}-{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&base);
    let input = base.join("input.txt");
    let packed = base.join("input.qzt");
    let sidecar_path = base.join("input.qzt.qzi");
    fs::write(&input, "東京大学\n京都大学\n").expect("input should be written");

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("pack")
            .arg(&input)
            .arg("-o")
            .arg(&packed),
    );
    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("sidecar-rebuild")
            .arg(&packed)
            .arg("-o")
            .arg(&sidecar_path)
            .arg("--index")
            .arg("ngram")
            .arg("--ngram")
            .arg("2"),
    );

    let mut sidecar = fs::read(&sidecar_path).expect("sidecar should be readable");
    corrupt(&mut sidecar);
    fs::write(&sidecar_path, &sidecar).expect("corrupted sidecar should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("search")
        .arg(&packed)
        .arg("東京")
        .arg("--sidecar")
        .arg(&sidecar_path)
        .output()
        .expect("search command should run");

    assert_eq!(
        output.status.code(),
        Some(1),
        "search must fail on corrupted sidecar ({label})"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.is_empty(),
        "stderr must contain a user-facing error ({label})"
    );
    assert!(
        !stderr.contains("panicked"),
        "corrupted sidecar must not panic ({label}): {stderr}"
    );

    assert_success(
        Command::new(env!("CARGO_BIN_EXE_qzt"))
            .arg("verify")
            .arg(&packed)
            .arg("--deep"),
    );

    let _ = fs::remove_dir_all(base);
}

fn patch_sidecar_first_term_flags(sidecar: &mut [u8], flags: u64) {
    let manifest_end = sidecar_manifest_end(sidecar);
    let (manifest_slice, rest) = sidecar.split_at_mut(manifest_end);
    let manifest = &mut manifest_slice[16..];
    let terms_region = manifest_section_region(manifest, b"terms", b"postings");
    let terms_offset =
        read_cbor_u64_after_key(terms_region, b"offset").expect("terms.offset should be readable");
    let terms_size =
        read_cbor_u64_after_key(terms_region, b"size").expect("terms.size should be readable");
    let terms_start = usize::try_from(terms_offset).expect("terms offset should fit usize");
    let terms_end = terms_start
        .checked_add(usize::try_from(terms_size).expect("terms size should fit usize"))
        .expect("terms end should fit");
    let terms = &mut rest[terms_start..terms_end];
    let flags_range = first_term_flags_byte_range(terms);
    terms[flags_range].copy_from_slice(&flags.to_le_bytes());
    let checksum = Checksum::blake3(terms).value;
    patch_manifest_section_checksum(terms_region, &checksum);
}

fn first_term_flags_byte_range(terms: &[u8]) -> std::ops::Range<usize> {
    let mut cursor = 8_usize;
    let key_len = usize::try_from(u64::from_le_bytes(
        terms[cursor..cursor + 8]
            .try_into()
            .expect("term key length bytes"),
    ))
    .expect("term key length should fit usize");
    cursor = cursor
        .checked_add(8)
        .and_then(|value| value.checked_add(key_len))
        .and_then(|value| value.checked_add(16))
        .and_then(|value| value.checked_add(8 * 6))
        .expect("first term flags offset should fit");
    cursor..cursor + 8
}

fn sidecar_manifest_end(sidecar: &[u8]) -> usize {
    const SIDECAR_HEADER_LEN: usize = 16;
    let manifest_size = usize::try_from(u64::from_le_bytes(
        sidecar[8..SIDECAR_HEADER_LEN]
            .try_into()
            .expect("manifest size bytes"),
    ))
    .expect("manifest size should fit usize");
    SIDECAR_HEADER_LEN
        .checked_add(manifest_size)
        .expect("manifest end should fit")
}

fn manifest_section_region<'a>(
    manifest: &'a mut [u8],
    section_key: &[u8],
    next_section_key: &[u8],
) -> &'a mut [u8] {
    let section_offset = manifest
        .windows(section_key.len())
        .position(|window| window == section_key)
        .expect("section key should exist in manifest");
    let next_offset = manifest[section_offset..]
        .windows(next_section_key.len())
        .position(|window| window == next_section_key)
        .expect("next section key should exist in manifest")
        + section_offset;
    &mut manifest[section_offset..next_offset]
}

fn read_cbor_u64_after_key(region: &[u8], key: &[u8]) -> Option<u64> {
    let key_offset = region.windows(key.len()).position(|window| window == key)?;
    let mut cursor = key_offset + key.len();
    let initial = *region.get(cursor)?;
    cursor += 1;
    match initial {
        0x00..=0x17 => Some(u64::from(initial)),
        0x18 => region.get(cursor).map(|value| u64::from(*value)),
        0x19 => {
            let bytes = region.get(cursor..cursor + 2)?;
            Some(u64::from(u16::from_be_bytes(bytes.try_into().ok()?)))
        }
        0x1a => {
            let bytes = region.get(cursor..cursor + 4)?;
            Some(u64::from(u32::from_be_bytes(bytes.try_into().ok()?)))
        }
        0x1b => {
            let bytes = region.get(cursor..cursor + 8)?;
            Some(u64::from_be_bytes(bytes.try_into().ok()?))
        }
        _ => None,
    }
}

fn patch_manifest_section_checksum(section_region: &mut [u8], checksum: &[u8; 32]) {
    let value_key = b"value";
    let value_key_offset = section_region
        .windows(value_key.len())
        .position(|window| window == value_key)
        .expect("section checksum value key should exist");
    let hash_start = value_key_offset + value_key.len();
    assert_eq!(
        section_region[hash_start], 0x58,
        "section checksum value must be a 32-byte CBOR byte string"
    );
    assert_eq!(section_region[hash_start + 1], 0x20);
    section_region[hash_start + 2..hash_start + 34].copy_from_slice(checksum);
}

fn patch_sidecar_granules_size_to_zero(sidecar: &mut [u8]) {
    const SIDECAR_HEADER_LEN: usize = 16;
    let manifest_size = usize::try_from(u64::from_le_bytes(
        sidecar[8..SIDECAR_HEADER_LEN]
            .try_into()
            .expect("manifest size bytes"),
    ))
    .expect("manifest size should fit usize");
    let manifest_start = SIDECAR_HEADER_LEN;
    let manifest_end = manifest_start
        .checked_add(manifest_size)
        .expect("manifest end should fit");
    let manifest = &mut sidecar[manifest_start..manifest_end];

    let granules_key = b"granules";
    let granules_offset = manifest
        .windows(granules_key.len())
        .position(|window| window == granules_key)
        .expect("granules key should exist in manifest");
    let postings_key = b"postings";
    let postings_offset = manifest[granules_offset..]
        .windows(postings_key.len())
        .position(|window| window == postings_key)
        .expect("postings key should exist in manifest")
        + granules_offset;
    let granules_region = &mut manifest[granules_offset..postings_offset];

    let size_key = b"size";
    let size_key_offset = granules_region
        .windows(size_key.len())
        .position(|window| window == size_key)
        .expect("granules.size key should exist");
    let size_value_offset = size_key_offset + size_key.len();
    assert_eq!(
        granules_region[size_value_offset], 0x08,
        "empty-input granules.size must be encoded as CBOR uint 8"
    );
    granules_region[size_value_offset] = 0x00;

    let value_key = b"value";
    let value_key_offset = granules_region
        .windows(value_key.len())
        .position(|window| window == value_key)
        .expect("granules checksum value key should exist");
    let hash_start = value_key_offset + value_key.len();
    assert_eq!(
        granules_region[hash_start], 0x58,
        "granules checksum value must be a 32-byte CBOR byte string"
    );
    assert_eq!(granules_region[hash_start + 1], 0x20);
    granules_region[hash_start + 2..hash_start + 34].copy_from_slice(&Checksum::blake3(b"").value);
}

fn flip_first_sidecar_payload_byte(sidecar: &mut [u8]) {
    const SIDECAR_HEADER_LEN: usize = 16;
    let manifest_size = usize::try_from(u64::from_le_bytes(
        sidecar[8..SIDECAR_HEADER_LEN]
            .try_into()
            .expect("manifest size bytes"),
    ))
    .expect("manifest size should fit usize");
    // Flip the first payload byte (after header + manifest) so checksum validation fails
    // rather than tripping missing-file or usage errors.
    let payload_offset = SIDECAR_HEADER_LEN
        .checked_add(manifest_size)
        .expect("payload offset should fit");
    assert!(
        payload_offset < sidecar.len(),
        "sidecar must have at least one payload byte"
    );
    sidecar[payload_offset] ^= 0xff;
}

fn patch_sidecar_schema(sidecar: &mut [u8]) {
    // Same-length mutation keeps CBOR manifest size stable for a focused binding test.
    replace_first(sidecar, b"qzt.sidecar.v1", b"qzt.sidecar.v9");
}

fn patch_sidecar_source_qzt_footer_checksum(sidecar: &mut [u8]) {
    let manifest_end = sidecar_manifest_end(sidecar);
    let manifest = &mut sidecar[16..manifest_end];
    let key = b"source_qzt_footer_checksum";
    let key_offset = manifest
        .windows(key.len())
        .position(|window| window == key)
        .expect("source_qzt_footer_checksum key should exist in manifest");
    let value_key = b"value";
    let value_key_offset = manifest[key_offset..]
        .windows(value_key.len())
        .position(|window| window == value_key)
        .expect("source_qzt_footer_checksum value key should exist")
        + key_offset;
    let hash_start = value_key_offset + value_key.len();
    assert_eq!(
        manifest[hash_start], 0x58,
        "source_qzt_footer_checksum value must be a 32-byte CBOR byte string"
    );
    assert_eq!(manifest[hash_start + 1], 0x20);
    manifest[hash_start + 2] ^= 0xff;
}

fn patch_sidecar_source_format_version(sidecar: &mut [u8], major: u8, minor: u8) {
    const SIDECAR_HEADER_LEN: usize = 16;
    let manifest_size = usize::try_from(u64::from_le_bytes(
        sidecar[8..SIDECAR_HEADER_LEN]
            .try_into()
            .expect("manifest size bytes"),
    ))
    .expect("manifest size should fit usize");
    let manifest_start = SIDECAR_HEADER_LEN;
    let manifest_end = manifest_start
        .checked_add(manifest_size)
        .expect("manifest end should fit");
    let manifest = &mut sidecar[manifest_start..manifest_end];
    let key = b"source_format_version";
    let key_offset = manifest
        .windows(key.len())
        .position(|window| window == key)
        .expect("source_format_version key should exist in manifest");
    let value_offset = key_offset + key.len();
    assert_eq!(
        manifest[value_offset], 0x82,
        "source_format_version must be encoded as a two-item CBOR array"
    );
    manifest[value_offset + 1] = major;
    manifest[value_offset + 2] = minor;
}

fn replace_first(bytes: &mut [u8], needle: &[u8], replacement: &[u8]) {
    assert_eq!(needle.len(), replacement.len());
    let start = bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("needle should exist");
    bytes[start..start + needle.len()].copy_from_slice(replacement);
}

#[test]
fn dense_sidecar_query_amortizes_physical_chunk_decodes() {
    let mut input = String::new();
    for index in 0..128 {
        let _ = writeln!(input, "aaa common {index}");
    }
    let container =
        pack_bytes_with_container_id(input.as_bytes(), [0xe6; 16], writer_options(64, 64))
            .expect("container should pack");
    let sidecar = build_search_sidecar(&container, SidecarIndexKind::Ngram { n: 3 })
        .expect("sidecar should build");
    let sidecar = QziSidecar::open(&container, &sidecar).expect("sidecar should open");
    let reader = QztReader::open(&container).expect("reader should open");

    let report = sidecar
        .search(&reader, "common", SearchOptions::default())
        .expect("sidecar search should run");

    assert_eq!(report.metrics.verified_matches, 128);
    assert!(report.metrics.physical_decoded_bytes > 0);
    assert!(report.metrics.physical_decoded_bytes <= reader.info().original_size);
}

#[test]
fn file_sidecar_search_matches_in_memory_sidecar_search() {
    let mut input = String::new();
    for index in 0..96 {
        let _ = writeln!(input, "info common line {index}");
    }
    input.push_str("alpha needle line\n");
    input.push_str("beta needle line\n");
    let container =
        pack_bytes_with_container_id(input.as_bytes(), [0xe7; 16], writer_options(128, 128))
            .expect("container should pack");

    for kind in [SidecarIndexKind::Token, SidecarIndexKind::Ngram { n: 3 }] {
        let sidecar_bytes = build_search_sidecar(&container, kind).expect("sidecar should build");
        let memory_sidecar =
            QziSidecar::open(&container, &sidecar_bytes).expect("sidecar should open");
        let memory_reader = QztReader::open(&container).expect("reader should open");
        let file_reader = QztFileReader::open_read_at(container.as_slice(), container.len() as u64)
            .expect("file reader should open");
        let file_sidecar = QziFileSidecar::open_read_at(
            sidecar_bytes.as_slice(),
            sidecar_bytes.len() as u64,
            &file_reader,
        )
        .expect("file sidecar should open");

        for query in ["needle", "needle line", "missingzzz"] {
            let memory = memory_sidecar
                .search(&memory_reader, query, SearchOptions::default())
                .expect("in-memory sidecar search should run");
            let file = file_sidecar
                .search(&file_reader, query, SearchOptions::default())
                .expect("file sidecar search should run");
            assert_semantic_report_eq(&memory, &file, &format!("query {query:?}"));
        }
    }
}

#[test]
fn file_sidecar_index_size_bytes_follows_serialized_manifest_model() {
    let mut non_skip_input = String::new();
    for index in 0..96 {
        let _ = writeln!(non_skip_input, "info common line {index}");
    }
    non_skip_input.push_str("alpha needle line\n");
    non_skip_input.push_str("beta needle line\n");
    let non_skip_container = pack_bytes_with_container_id(
        non_skip_input.as_bytes(),
        [0xea; 16],
        writer_options(128, 128),
    )
    .expect("container should pack");
    let non_skip_sidecar =
        build_search_sidecar(&non_skip_container, SidecarIndexKind::Ngram { n: 3 })
            .expect("sidecar should build");
    let non_skip_memory =
        QziSidecar::open(&non_skip_container, &non_skip_sidecar).expect("sidecar should open");
    let non_skip_reader = QztReader::open(&non_skip_container).expect("reader should open");
    let non_skip_file_reader = QztFileReader::open_read_at(
        non_skip_container.as_slice(),
        non_skip_container.len() as u64,
    )
    .expect("file reader should open");
    let non_skip_file = QziFileSidecar::open_read_at(
        non_skip_sidecar.as_slice(),
        non_skip_sidecar.len() as u64,
        &non_skip_file_reader,
    )
    .expect("file sidecar should open");

    let non_skip_memory_report = non_skip_memory
        .search(&non_skip_reader, "line", SearchOptions::default())
        .expect("in-memory sidecar search should run");
    let non_skip_file_report = non_skip_file
        .search(&non_skip_file_reader, "line", SearchOptions::default())
        .expect("file sidecar search should run");

    assert_eq!(
        non_skip_memory_report.metrics.index_size_bytes + 16,
        non_skip_file_report.metrics.index_size_bytes,
        "non-skip query should keep the +16 serialized header delta"
    );

    let mut skip_input = String::new();
    for index in 0..1100 {
        let _ = writeln!(skip_input, "aaa line {index}");
    }
    let skip_container =
        pack_bytes_with_container_id(skip_input.as_bytes(), [0xeb; 16], writer_options(512, 512))
            .expect("container should pack");
    let skip_sidecar = build_search_sidecar(&skip_container, SidecarIndexKind::Ngram { n: 3 })
        .expect("sidecar should build");
    let skip_memory =
        QziSidecar::open(&skip_container, &skip_sidecar).expect("sidecar should open");
    let skip_reader = QztReader::open(&skip_container).expect("reader should open");
    let skip_file_reader =
        QztFileReader::open_read_at(skip_container.as_slice(), skip_container.len() as u64)
            .expect("file reader should open");
    let skip_file = QziFileSidecar::open_read_at(
        skip_sidecar.as_slice(),
        skip_sidecar.len() as u64,
        &skip_file_reader,
    )
    .expect("file sidecar should open");

    let skip_memory_report = skip_memory
        .search(&skip_reader, "aaa", SearchOptions::default())
        .expect("in-memory sidecar search should run");
    let skip_file_report = skip_file
        .search(&skip_file_reader, "aaa", SearchOptions::default())
        .expect("file sidecar search should run");

    assert!(
        skip_file_report.metrics.index_size_bytes < skip_memory_report.metrics.index_size_bytes,
        "skip-list encoding causes in-memory estimate to exceed serialized manifest size"
    );
}

#[test]
fn file_sidecar_search_reads_lazily_from_sidecar() {
    let mut input = String::new();
    for index in 0..256 {
        let _ = writeln!(input, "info line number {index}");
    }
    input.push_str("zzztarget unique\n");
    let container =
        pack_bytes_with_container_id(input.as_bytes(), [0xe8; 16], writer_options(512, 512))
            .expect("container should pack");
    let sidecar_bytes = build_search_sidecar(&container, SidecarIndexKind::Ngram { n: 3 })
        .expect("sidecar should build");
    let file_reader = QztFileReader::open_read_at(container.as_slice(), container.len() as u64)
        .expect("file reader should open");

    let counting = CountingReadAt::new(sidecar_bytes.clone());
    let reads = counting.reads.clone();
    let sidecar = QziFileSidecar::open_read_at(counting, sidecar_bytes.len() as u64, &file_reader)
        .expect("file sidecar should open");
    reads.lock().expect("reads lock").clear();

    let report = sidecar
        .search(&file_reader, "zzz", SearchOptions::default())
        .expect("file sidecar search should run");
    assert_eq!(report.metrics.verified_matches, 1);

    // One tiny posting list plus one 56-byte granule record. An eager open
    // would have decoded the full posting and granule sections (tens of KiB
    // for this corpus) instead.
    let query_reads = reads.lock().expect("reads lock").clone();
    let total: u64 = query_reads.iter().map(|(_, size)| *size).sum();
    assert!(
        query_reads.len() <= 4,
        "expected a handful of lazy reads, got {query_reads:?}"
    );
    assert!(total < 1024, "expected lazy reads, read {total} bytes");
}

#[test]
fn file_sidecar_open_rejects_wrong_source_container() {
    let container_a = pack_bytes_with_container_id(b"alpha\n", [0xe9; 16], writer_options(64, 64))
        .expect("container a should pack");
    let container_b = pack_bytes_with_container_id(b"alpha\n", [0xea; 16], writer_options(64, 64))
        .expect("container b should pack");
    let sidecar_bytes =
        build_search_sidecar(&container_a, SidecarIndexKind::Token).expect("sidecar should build");
    let reader_b = QztFileReader::open_read_at(container_b.as_slice(), container_b.len() as u64)
        .expect("file reader should open");

    let error = QziFileSidecar::open_read_at(
        sidecar_bytes.as_slice(),
        sidecar_bytes.len() as u64,
        &reader_b,
    )
    .err();
    assert_eq!(error, Some(QztError::ContainerIdMismatch));
}
