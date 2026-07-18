use std::fs;
use std::path::Path;

#[path = "support/benchmark_log.rs"]
mod benchmark_log;

const REPORT: &str = include_str!("../docs/benchmarks/2026-07-v0.1.md");
const README_EN: &str = include_str!("../README.md");
const README_JA: &str = include_str!("../README.ja.md");
const METHODOLOGY: &str = include_str!("../docs/QZT_v0.1_Competitive_Benchmarks.md");
const RELEASE_HARDENING_JA: &str = include_str!("../docs/QZT_v0.1_Release_Hardening.ja.md");

fn number(line: &str, key: &str) -> u64 {
    benchmark_log::field(line, key)
        .parse()
        .unwrap_or_else(|error| panic!("invalid {key} in benchmark record: {error}"))
}

fn comma_separated(value: u64) -> String {
    let digits = value.to_string();
    let mut rendered = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            rendered.push(',');
        }
        rendered.push(digit);
    }
    rendered
}

#[test]
fn current_competitive_log_contract_round_trips_through_publication_parser() {
    let fields = [
        ("corpus_id", "C2".to_owned()),
        ("qzt_range_micros", "17".to_owned()),
        ("ripgrep_hit_count", "504".to_owned()),
    ];
    let record = benchmark_log::format_record(&fields);
    assert!(record.starts_with("competitive_benchmark "));
    assert_eq!(benchmark_log::field(&record, "corpus_id"), "C2");
    assert_eq!(number(&record, "qzt_range_micros"), 17);
    assert_eq!(number(&record, "ripgrep_hit_count"), 504);
}

#[test]
fn benchmark_report_has_reproducible_and_honest_structure() {
    for heading in [
        "## TL;DR",
        "## Environment",
        "## Methodology",
        "## Results",
        "### 1. Range restore: QZT vs whole-file zstd",
        "### 2. Query latency",
        "### 3. Search correctness cross-check",
        "### 4. Sizes",
        "## Honest Reading",
        "### When not to use QZT",
        "## Reproduce",
    ] {
        assert!(
            REPORT.contains(heading),
            "missing report heading: {heading}"
        );
    }

    for boundary in [
        "not an SLA",
        "Tantivy",
        "Lucene",
        "seekable-zstd",
        "pre-built index",
        "build-time memory",
        "phrase search",
    ] {
        assert!(
            REPORT.contains(boundary),
            "missing honesty boundary: {boundary}"
        );
    }
}

#[test]
fn report_links_to_three_complete_raw_runs() {
    let raw_root = Path::new("docs/benchmarks/raw/2026-07-v0.1");
    assert!(raw_root.join("environment.txt").is_file());

    for run in 1..=3 {
        for prefix in ["bench-release", "profile-matrix", "bench-compete"] {
            let relative = format!("raw/2026-07-v0.1/{prefix}-run-{run}.log");
            assert!(REPORT.contains(&relative), "report must link {relative}");

            let log = fs::read_to_string(raw_root.join(format!("{prefix}-run-{run}.log")))
                .unwrap_or_else(|error| panic!("missing raw run for {prefix} #{run}: {error}"));
            assert!(
                log.contains("test result: ok"),
                "raw run did not pass: {relative}"
            );
        }

        let matrix = fs::read_to_string(raw_root.join(format!("profile-matrix-run-{run}.log")))
            .expect("matrix raw log must exist");
        assert_eq!(matrix.matches("[profile-matrix]").count(), 9);
        assert_eq!(matrix.matches("reps=500 warmup=20").count(), 9);

        let compete = fs::read_to_string(raw_root.join(format!("bench-compete-run-{run}.log")))
            .expect("competitive raw log must exist");
        assert!(compete.contains("ripgrep_hit_count=504"));
        assert!(compete.contains("sqlite_fts5_hit_count=504"));
    }
}

#[test]
fn published_numbers_are_derived_from_retained_raw_logs() {
    let raw_root = Path::new("docs/benchmarks/raw/2026-07-v0.1");
    let environment = fs::read_to_string(raw_root.join("environment.txt"))
        .expect("environment snapshot must exist");
    let commit = environment
        .lines()
        .find_map(|line| line.strip_prefix("Repository commit: "))
        .expect("environment must record the benchmark commit");
    assert!(REPORT.contains(commit));

    let mut competitive_records = Vec::new();
    let mut matrix_logs = Vec::new();
    for run in 1..=3 {
        let competitive = fs::read_to_string(raw_root.join(format!("bench-compete-run-{run}.log")))
            .expect("competitive raw log must exist");
        competitive_records.push(
            competitive
                .lines()
                .find(|line| line.starts_with("competitive_benchmark "))
                .expect("competitive log must contain a machine-readable record")
                .to_owned(),
        );
        matrix_logs.push(
            fs::read_to_string(raw_root.join(format!("profile-matrix-run-{run}.log")))
                .expect("matrix raw log must exist"),
        );
    }

    for (index, record) in competitive_records.iter().enumerate() {
        let expected_range_row = format!(
            "| {} | {} µs | {} µs | {} / {} |",
            index + 1,
            number(record, "qzt_range_micros"),
            number(record, "raw_zstd_range_micros"),
            comma_separated(number(record, "qzt_range_bytes")),
            comma_separated(number(record, "raw_zstd_decoded_bytes")),
        );
        assert!(
            REPORT.contains(&expected_range_row),
            "range row differs from raw run {}: {expected_range_row}",
            index + 1
        );

        let expected_correctness_row = format!(
            "| {} | {} | {} | {} | {} |",
            index + 1,
            number(record, "token_hit_count"),
            number(record, "reference_hit_count"),
            number(record, "ripgrep_hit_count"),
            number(record, "sqlite_fts5_hit_count"),
        );
        assert!(REPORT.contains(&expected_correctness_row));
    }
    let qzt_favored_runs: Vec<usize> = competitive_records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            (number(record, "qzt_range_micros") < number(record, "raw_zstd_range_micros"))
                .then_some(index + 1)
        })
        .collect();
    // The prose is a derived claim too. Freeze it alongside the table so a
    // fresh timing run cannot silently leave the interpretation stale.
    assert_eq!(qzt_favored_runs, vec![3]);
    assert!(REPORT.contains("Only run 3 favored QZT; runs 1 and 2 favored whole-file zstd"));

    for (size, size_label) in [("1MB", "1 MB"), ("10MB", "10 MB"), ("100MB", "100 MB")] {
        for (kind, kind_label) in [
            ("ascii", "ASCII"),
            ("utf8-mixed", "UTF-8 mixed"),
            ("japanese", "Japanese"),
        ] {
            let marker = format!("[profile-matrix] corpus={size} kind={kind} ");
            let records: Vec<&str> = matrix_logs
                .iter()
                .map(|log| {
                    log.lines()
                        .collect::<Vec<_>>()
                        .windows(2)
                        .find_map(|pair| {
                            (pair[0].starts_with(&marker) && pair[1].starts_with("release_bench "))
                                .then_some(pair[1])
                        })
                        .unwrap_or_else(|| {
                            panic!("matrix marker must be adjacent to its record: {marker}")
                        })
                })
                .collect();

            let percentiles: Vec<(u64, u64, u64)> = records
                .iter()
                .map(|record| {
                    let common = record
                        .split("common_ngram_query=\"")
                        .nth(1)
                        .expect("release record must contain common n-gram query");
                    (
                        number(common, "p50_us"),
                        number(common, "p95_us"),
                        number(common, "p99_us"),
                    )
                })
                .collect();
            let range = |position: usize| {
                let values: Vec<u64> = percentiles
                    .iter()
                    .map(|values| [values.0, values.1, values.2][position])
                    .collect();
                format!(
                    "{}–{}",
                    comma_separated(*values.iter().min().expect("three matrix runs")),
                    comma_separated(*values.iter().max().expect("three matrix runs")),
                )
            };
            let expected_latency_row = format!(
                "| {size_label} | {kind_label} | {} | {} | {} |",
                range(0),
                range(1),
                range(2),
            );
            assert!(
                REPORT.contains(&expected_latency_row),
                "latency row differs from retained matrix runs: {expected_latency_row}"
            );

            // Size ratios are deterministic; format the first retained run exactly
            // as the publication does, while the three-run equality is checked below.
            for key in ["compression_ratio", "qzi_token_ratio", "qzi_ngram_ratio"] {
                let first = benchmark_log::field(records[0], key);
                assert!(
                    records
                        .iter()
                        .all(|record| benchmark_log::field(record, key) == first)
                );
            }
            let expected_size_row = format!(
                "| {size_label} | {kind_label} | {:.3} | {:.3} | {:.3} |",
                benchmark_log::field(records[0], "compression_ratio")
                    .parse::<f64>()
                    .unwrap(),
                benchmark_log::field(records[0], "qzi_token_ratio")
                    .parse::<f64>()
                    .unwrap(),
                benchmark_log::field(records[0], "qzi_ngram_ratio")
                    .parse::<f64>()
                    .unwrap(),
            );
            assert!(REPORT.contains(&expected_size_row));
        }
    }

    let first = &competitive_records[0];
    assert!(REPORT.contains(&format!(
        "QZT was {} bytes",
        comma_separated(number(first, "qzt_bytes"))
    )));
    assert!(REPORT.contains(&format!(
        "raw zstd\nwas {} bytes",
        comma_separated(number(first, "raw_zstd_bytes"))
    )));
}

#[test]
fn report_does_not_overstate_partial_restore_evidence() {
    for unsupported_claim in [
        "Decoded bytes QZT / zstd",
        "64x fewer",
        "(cold)",
        "warm runs",
    ] {
        assert!(
            !REPORT.contains(unsupported_claim),
            "unsupported claim: {unsupported_claim}"
        );
    }
    assert!(REPORT.contains("QZT returned / zstd decoded"));
    assert!(REPORT.contains("already-open"));
    assert!(REPORT.contains("CLI startup"));
    assert!(REPORT.contains("does not measure\nthat decoded amount"));
}

#[test]
fn readmes_publish_evidence_without_erasing_unmeasured_comparators() {
    for readme in [README_EN, README_JA] {
        assert!(readme.contains("docs/benchmarks/2026-07-v0.1.md"));
        assert!(readme.contains("ripgrep"));
        assert!(readme.contains("SQLite FTS5"));
        assert!(readme.contains("Tantivy"));
        assert!(readme.contains("Lucene"));
        assert!(readme.contains("seekable-zstd"));
    }

    assert!(!README_EN.contains("**No production benchmark**"));
    assert!(!README_JA.contains("**Production benchmarkは未実施**"));
    for document in [README_EN, README_JA, METHODOLOGY, REPORT] {
        assert!(!document.contains("cargo test --features bench-compete"));
        assert!(document.contains("--all-features"));
    }
    assert!(RELEASE_HARDENING_JA.contains("benchmarks/2026-07-v0.1.md"));
    assert!(!RELEASE_HARDENING_JA.contains("競合 benchmark は次の"));
}
