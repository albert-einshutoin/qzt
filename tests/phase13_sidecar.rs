use std::fs;
use std::process::Command;

use qzt::chunker::ChunkerOptions;
use qzt::error::QztError;
use qzt::reader::{QztReader, VerifyLevel};
use qzt::schema::Checksum;
use qzt::search::{NgramIndexBuildOptions, RawNgramIndex, SearchOptions};
use qzt::sidecar::{build_search_sidecar, QziSidecar, SidecarIndexKind};
use qzt::writer::{pack_bytes_with_container_id, WriterOptions};

fn options(target_chunk_size: usize, max_chunk_size: usize) -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size,
            max_chunk_size,
        },
        zstd_level: 0,
    }
}

#[test]
fn wrong_source_container_id_sidecar_is_rejected() {
    let input = b"alpha\n";
    let container = pack_bytes_with_container_id(input, [0xe0; 16], options(64, 64))
        .expect("container should pack");
    let mut sidecar =
        build_search_sidecar(&container, SidecarIndexKind::Token).expect("sidecar should build");
    replace_first(&mut sidecar, &[0xe0; 16], &[0xee; 16]);

    assert_eq!(
        QziSidecar::open(&container, &sidecar).map(|_| ()),
        Err(QztError::ContainerIdMismatch)
    );
}

#[test]
fn wrong_source_original_checksum_sidecar_is_rejected() {
    let input = b"alpha\n";
    let container = pack_bytes_with_container_id(input, [0xe1; 16], options(64, 64))
        .expect("container should pack");
    let mut sidecar =
        build_search_sidecar(&container, SidecarIndexKind::Token).expect("sidecar should build");
    let checksum = Checksum::blake3(input).value;
    let mut wrong = checksum;
    wrong[0] ^= 0xff;
    replace_first(&mut sidecar, &checksum, &wrong);

    assert_eq!(
        QziSidecar::open(&container, &sidecar).map(|_| ()),
        Err(QztError::ContainerCorrupt)
    );
}

#[test]
fn missing_or_rejected_sidecar_does_not_break_core_operations() {
    let input = b"alpha\nbeta\n";
    let container = pack_bytes_with_container_id(input, [0xe2; 16], options(8, 8))
        .expect("container should pack");
    let reader = QztReader::open(&container).expect("reader should open without sidecar");

    assert_eq!(reader.export_all().expect("export should work"), input);
    assert!(reader.verify(VerifyLevel::Deep).is_ok());
}

#[test]
fn sidecar_lookup_matches_transient_ngram_index_behavior() {
    let input = "東京大学\n京都大学\n";
    let container = pack_bytes_with_container_id(input.as_bytes(), [0xe3; 16], options(8, 8))
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

    assert_eq!(sidecar_report.hits, transient_report.hits);
    assert_eq!(
        sidecar_report.metrics.candidate_granules,
        transient_report.metrics.candidate_granules
    );
    assert_eq!(
        sidecar_report.metrics.decoded_bytes,
        transient_report.metrics.decoded_bytes
    );
}

#[test]
fn common_term_sidecar_query_is_capped_without_decoding_candidates() {
    let mut input = String::new();
    for index in 0..128 {
        input.push_str(&format!("aaa common {index}\n"));
    }
    let container = pack_bytes_with_container_id(input.as_bytes(), [0xe4; 16], options(256, 256))
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
        input.push_str(&format!("info line {index}\n"));
    }
    input.push_str("zzztarget\n");
    let container = pack_bytes_with_container_id(input.as_bytes(), [0xe5; 16], options(64, 64))
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

fn replace_first(bytes: &mut [u8], needle: &[u8], replacement: &[u8]) {
    assert_eq!(needle.len(), replacement.len());
    let start = bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("needle should exist");
    bytes[start..start + needle.len()].copy_from_slice(replacement);
}

fn output_success(command: &mut Command) -> Vec<u8> {
    let output = command.output().expect("command should run");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn assert_success(command: &mut Command) {
    let output = command.output().expect("command should run");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn dense_sidecar_query_amortizes_physical_chunk_decodes() {
    let mut input = String::new();
    for index in 0..128 {
        input.push_str(&format!("aaa common {index}\n"));
    }
    let container = pack_bytes_with_container_id(input.as_bytes(), [0xe6; 16], options(64, 64))
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
