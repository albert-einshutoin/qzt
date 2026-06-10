use std::fs;
use std::process::Command;

use qzt::chunker::ChunkerOptions;
use qzt::error::QztError;
use qzt::reader::QztFileReader;
use qzt::reader::{QztReader, VerifyLevel};
use qzt::schema::Checksum;
use qzt::search::{NgramIndexBuildOptions, RawNgramIndex, SearchOptions};
use qzt::sidecar::{build_search_sidecar, QziFileSidecar, QziSidecar, SidecarIndexKind};
use qzt::writer::{pack_bytes_with_container_id, WriterOptions};
mod phase13_search_report;
use phase13_search_report::assert_semantic_report_eq;

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

    assert_semantic_report_eq(&sidecar_report, &transient_report, "sidecar vs transient");
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

#[test]
fn file_sidecar_search_matches_in_memory_sidecar_search() {
    let mut input = String::new();
    for index in 0..96 {
        input.push_str(&format!("info common line {index}\n"));
    }
    input.push_str("alpha needle line\n");
    input.push_str("beta needle line\n");
    let container = pack_bytes_with_container_id(input.as_bytes(), [0xe7; 16], options(128, 128))
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
fn file_sidecar_search_reads_lazily_from_sidecar() {
    let mut input = String::new();
    for index in 0..256 {
        input.push_str(&format!("info line number {index}\n"));
    }
    input.push_str("zzztarget unique\n");
    let container = pack_bytes_with_container_id(input.as_bytes(), [0xe8; 16], options(512, 512))
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
    let container_a = pack_bytes_with_container_id(b"alpha\n", [0xe9; 16], options(64, 64))
        .expect("container a should pack");
    let container_b = pack_bytes_with_container_id(b"alpha\n", [0xea; 16], options(64, 64))
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

struct CountingReadAt {
    bytes: std::sync::Arc<Vec<u8>>,
    reads: std::sync::Arc<std::sync::Mutex<Vec<(u64, u64)>>>,
}

impl CountingReadAt {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: std::sync::Arc::new(bytes),
            reads: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

impl qzt::io::ReadAt for CountingReadAt {
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
        self.reads
            .lock()
            .map_err(|_| std::io::Error::other("poisoned reads lock"))?
            .push((offset, buf.len() as u64));
        let start = usize::try_from(offset).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "offset too large")
        })?;
        let end = start.checked_add(buf.len()).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "range overflow")
        })?;
        let source = self
            .bytes
            .get(start..end)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "short read"))?;
        buf.copy_from_slice(source);
        Ok(())
    }
}
