use qzt::{
    CorpusKind, PartialDecompressionBenchmarkOptions, QztFileReader, QztReader,
    open_skeleton_details, pack_bytes_with_container_id, run_partial_decompression_benchmark,
};

mod support;
use support::{CountingReadAt, writer_options};

const MAKEFILE: &str = include_str!("../Makefile");
const PRODUCTION_EVIDENCE: &str =
    include_str!("../docs/benchmarks/2026-07-partial-decompression.md");
const ISOLATED_PROBE: &str = include_str!("../examples/partial_decompression_probe.rs");
const README_EN: &str = include_str!("../README.md");
const README_JA: &str = include_str!("../README.ja.md");
const PRODUCTION_RUNS: [&str; 3] = [
    include_str!("../docs/benchmarks/raw/2026-07-partial-decompression/production-run-1.log"),
    include_str!("../docs/benchmarks/raw/2026-07-partial-decompression/production-run-2.log"),
    include_str!("../docs/benchmarks/raw/2026-07-partial-decompression/production-run-3.log"),
];
const RSS_RUN: &str =
    include_str!("../docs/benchmarks/raw/2026-07-partial-decompression/rss-probe-run-1.log");

#[test]
fn range_metrics_measure_only_the_single_intersecting_chunk() {
    let input = b"aaaaaaa\nbbbbbbb\nccccccc\nddddddd\n";
    let container = pack_bytes_with_container_id(input, [0x46; 16], writer_options(8, 8))
        .expect("pack should work");
    let details = open_skeleton_details(&container).expect("container should open");
    let counting = CountingReadAt::new(container.clone());
    let reads = counting.reads.clone();
    let reader =
        QztFileReader::open_read_at(counting, container.len() as u64).expect("open should work");
    reads.lock().expect("reads lock").clear();

    let report = reader
        .read_range_with_metrics(9, 2)
        .expect("range should read");

    assert_eq!(report.bytes, &input[9..11]);
    assert_eq!(report.metrics.decoded_chunks, 1);
    assert_eq!(report.metrics.decoded_bytes, 8);
    assert_eq!(
        report.metrics.compressed_bytes,
        details.chunk_entries[1].compressed_size
    );
    let physical_payload_bytes = reads
        .lock()
        .expect("reads lock")
        .iter()
        .map(|(_, size)| *size)
        .sum::<u64>();
    assert_eq!(physical_payload_bytes, report.metrics.compressed_bytes);
}

#[test]
fn range_metrics_count_both_chunks_crossed_by_a_boundary_range() {
    let input = b"aaaaaaa\nbbbbbbb\nccccccc\nddddddd\n";
    let container = pack_bytes_with_container_id(input, [0x47; 16], writer_options(8, 8))
        .expect("pack should work");
    let details = open_skeleton_details(&container).expect("container should open");
    let reader = QztReader::open(&container).expect("reader should open");

    let report = reader
        .read_range_with_metrics(14, 4)
        .expect("boundary range should read");

    assert_eq!(report.bytes, &input[14..18]);
    assert_eq!(report.metrics.decoded_chunks, 2);
    assert_eq!(report.metrics.decoded_bytes, 16);
    assert_eq!(
        report.metrics.compressed_bytes,
        details.chunk_entries[1].compressed_size + details.chunk_entries[2].compressed_size
    );
}

#[test]
fn empty_range_reports_zero_work() {
    let container =
        pack_bytes_with_container_id(b"alpha\nbeta\n", [0x48; 16], writer_options(8, 8))
            .expect("pack should work");
    let reader = QztReader::open(&container).expect("reader should open");

    let report = reader
        .read_range_with_metrics(5, 0)
        .expect("empty range should read");

    assert!(report.bytes.is_empty());
    assert_eq!(report.metrics.decoded_chunks, 0);
    assert_eq!(report.metrics.decoded_bytes, 0);
    assert_eq!(report.metrics.compressed_bytes, 0);
}

#[test]
fn partial_decompression_probe_records_bounded_work_on_a_scalable_corpus() {
    let corpus_bytes = std::env::var("QZT_PARTIAL_BENCH_CORPUS_BYTES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(8 * 1024 * 1024);
    let range_size = 64 * 1024_u64;
    let report = run_partial_decompression_benchmark(PartialDecompressionBenchmarkOptions {
        corpus_kind: CorpusKind::C2Logs,
        corpus_bytes,
        chunk_size: 256 * 1024,
        range_offset: (corpus_bytes as u64).saturating_mul(3) / 4,
        range_size,
    })
    .expect("partial-decompression benchmark should run");

    assert!(report.corpus_bytes >= corpus_bytes as u64);
    assert_eq!(report.returned_bytes, range_size);
    assert!(report.decoded_chunks <= 2);
    assert!(report.decoded_bytes <= 2 * 256 * 1024);
    assert!(report.decoded_bytes < report.corpus_bytes);
    assert!(report.compressed_bytes < report.qzt_bytes);

    eprintln!("{report}");
}

#[test]
fn production_probe_has_a_reproducible_and_honest_publication_contract() {
    assert!(MAKEFILE.contains("bench-partial-decompression:"));
    assert!(MAKEFILE.contains("QZT_PARTIAL_BENCH_CORPUS_BYTES"));
    for required in [
        "1 GiB",
        "decoded_bytes",
        "compressed_bytes",
        "not an SLA",
        "QZI",
        "make bench-partial-decompression",
        "raw/2026-07-partial-decompression/production-run-1.log",
        "maximum resident set size",
        "partial_decompression_probe",
    ] {
        assert!(
            PRODUCTION_EVIDENCE.contains(required),
            "missing evidence boundary: {required}"
        );
    }
    assert!(ISOLATED_PROBE.contains("QztFileReader::open_path"));
    assert!(!ISOLATED_PROBE.contains("read_to_end"));
    for readme in [README_EN, README_JA] {
        assert!(readme.contains("docs/benchmarks/2026-07-partial-decompression.md"));
    }
    for evidence in [PRODUCTION_EVIDENCE, RSS_RUN] {
        for command_fragment in [
            "dd if=/dev/zero",
            "--chunk-size 16777216",
            "--max-chunk-size 16777216",
            "qzt info",
            "/usr/bin/time -l",
        ] {
            assert!(
                evidence.contains(command_fragment),
                "missing isolated-probe command: {command_fragment}"
            );
        }
    }
}

#[test]
fn published_structural_metrics_are_derived_from_all_retained_runs() {
    for log in PRODUCTION_RUNS {
        assert!(log.contains("test result: ok"));
        let record = log
            .lines()
            .find(|line| line.starts_with("partial_decompression_benchmark "))
            .expect("raw run must contain a machine-readable record");
        assert_eq!(record_field(record, "corpus_bytes"), "1073741824");
        assert_eq!(record_field(record, "returned_bytes"), "65536");
        assert_eq!(record_field(record, "decoded_chunks"), "1");
        assert_eq!(record_field(record, "decoded_bytes"), "262085");
        assert_eq!(record_field(record, "compressed_bytes"), "14339");
    }

    assert!(PRODUCTION_EVIDENCE.contains("65,536 B | 262,085 B | 14,339 B | 1"));
    for metadata in [
        "\"target_chunk_size\": 16777216",
        "\"max_chunk_size\": 16777216",
        "\"chunk_count\": 64",
        "decoded_bytes=16777216",
        "compressed_bytes=530",
        "21168128  maximum resident set size",
    ] {
        assert!(
            RSS_RUN.contains(metadata),
            "missing RSS evidence: {metadata}"
        );
    }
    assert!(
        PRODUCTION_EVIDENCE.contains("21,168,128 B (20.19 MiB)"),
        "RSS publication must match the retained process output"
    );
}

fn record_field<'a>(record: &'a str, key: &str) -> &'a str {
    let prefix = format!("{key}=");
    record
        .split_whitespace()
        .find_map(|field| field.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("missing {key} in benchmark record"))
}
