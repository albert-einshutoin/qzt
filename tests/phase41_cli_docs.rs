use std::collections::BTreeSet;
use std::fs;
use std::process::{Command, Output};

use qzt::{DocumentSpan, SearchOptions, WriterBuilder, WriterOptions, pack_bytes};

const ENGLISH: &str = include_str!("../docs/CLI.md");
const JAPANESE: &str = include_str!("../docs/CLI.ja.md");
const MAIN_SOURCE: &str = include_str!("../src/main.rs");

#[test]
fn english_and_japanese_references_cover_every_command_and_option() {
    for document in [ENGLISH, JAPANESE] {
        for command in [
            "qzt help",
            "qzt pack <INPUT|->",
            "qzt pack-docs <INPUT>...",
            "qzt info <FILE>",
            "qzt export <FILE>",
            "qzt range <FILE>",
            "qzt line <FILE>",
            "qzt docs <FILE>",
            "qzt doc <FILE>",
            "qzt search <FILE>",
            "qzt sidecar-rebuild <FILE>",
            "qzt verify <FILE>",
            "qzt attest",
        ] {
            assert!(document.contains(command), "missing command {command}");
        }
        for option in [
            "--output",
            "--version",
            "--profile",
            "--chunk-size",
            "--max-chunk-size",
            "--zstd-level",
            "--checksum blake3",
            "--dict none",
            "--dense-line-index",
            "--doc-id-prefix",
            "--format text|json",
            "--bytes A:B",
            "--lines A:B",
            "--zero-based",
            "--no-verify",
            "--index token|ngram",
            "--ngram <N>",
            "--sidecar <PATH>",
            "--max-candidates",
            "--max-decoded-bytes",
            "--max-results",
            "--quick|--normal|--deep",
            "--level quick|normal|deep",
        ] {
            assert!(document.contains(option), "missing option {option}");
        }
    }
}

#[test]
fn references_freeze_automation_boundaries_without_overclaiming_text() {
    for document in [ENGLISH, JAPANESE] {
        for exit_code in ["`0`", "`1`", "`2`"] {
            assert!(document.contains(exit_code));
        }
        for json_field in [
            "container_id",
            "original_checksum",
            "checked_chunks",
            "decoded_bytes",
            "incomplete_reason",
            "logical_offset",
            "document_count",
        ] {
            assert!(
                document.contains(json_field),
                "missing JSON field {json_field}"
            );
        }
        assert!(document.contains("stdout"));
        assert!(document.contains("stderr"));
        assert!(document.contains("query_time_ms"));
        assert!(document.contains("missing_required_key_in_incomplete_index"));
        assert!(document.contains("target/debug/qzt"));
        assert!(document.contains("technical preview"));
    }
}

#[test]
fn readmes_link_to_the_language_matching_reference() {
    let english_readme = include_str!("../README.md");
    let japanese_readme = include_str!("../README.ja.md");
    assert!(english_readme.contains("[docs/CLI.md](docs/CLI.md)"));
    assert!(japanese_readme.contains("[docs/CLI.ja.md](docs/CLI.ja.md)"));
}

#[test]
fn documented_command_set_matches_the_cli_dispatch_and_outlines_match() {
    let dispatched = MAIN_SOURCE
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("Some(\"")?;
            let (command, suffix) = rest.split_once('"')?;
            suffix.contains("=> run_").then_some(command)
        })
        .collect::<Vec<_>>();
    assert_eq!(dispatched.len(), 12, "new dispatch arms require CLI docs");
    for command in dispatched {
        let signature = format!("`qzt {command}");
        assert!(ENGLISH.contains(&signature), "English docs miss {command}");
        assert!(
            JAPANESE.contains(&signature),
            "Japanese docs miss {command}"
        );
    }

    let outline = |document: &str| {
        document
            .lines()
            .filter(|line| line.starts_with("## ") || line.starts_with("### "))
            .map(|line| {
                line.chars()
                    .take_while(|character| *character == '#')
                    .count()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(outline(ENGLISH), outline(JAPANESE));
}

#[test]
fn every_operational_command_rejects_missing_and_unknown_arguments_on_stderr() {
    let fixture = CliFixture::new();
    let packed = fixture.packed.to_str().unwrap();
    let documents = fixture.documents.to_str().unwrap();
    let output = fixture.base.join("unused.out");
    let output = output.to_str().unwrap();

    let unknown_cases = [
        vec!["pack", packed, "-o", output, "--bogus"],
        vec!["pack-docs", packed, "-o", output, "--bogus"],
        vec!["info", packed, "--bogus"],
        vec!["export", packed, "--bogus"],
        vec!["range", packed, "--bytes", "0:1", "--bogus"],
        vec!["line", packed, "1", "--bogus"],
        vec!["docs", documents, "--bogus"],
        vec!["doc", documents, "doc-1", "--bogus"],
        vec!["search", packed, "alpha", "--bogus"],
        vec!["sidecar-rebuild", packed, "-o", output, "--bogus"],
        vec!["verify", packed, "--bogus"],
        vec!["attest", packed, "--bogus"],
    ];
    for arguments in unknown_cases {
        assert_usage_error(&arguments, "unknown option");
    }

    for command in [
        "pack",
        "pack-docs",
        "info",
        "export",
        "range",
        "line",
        "docs",
        "doc",
        "search",
        "sidecar-rebuild",
        "verify",
        "attest",
    ] {
        assert_usage_error(&[command], "missing");
    }
}

#[test]
fn machine_readable_schemas_and_defaults_match_the_documented_contract() {
    let fixture = CliFixture::new();
    let packed = fixture.packed.to_str().unwrap();
    let documents = fixture.documents.to_str().unwrap();

    let info = run_json(&["info", packed, "--format", "json"]);
    assert_keys(
        &info,
        &[
            "chunk_count",
            "compressed_size",
            "container_id",
            "dense_line_index",
            "document_count",
            "document_index",
            "format",
            "line_count",
            "max_chunk_size",
            "newline_mode",
            "original_checksum",
            "original_size",
            "profile",
            "target_chunk_size",
            "zstd_level",
        ],
    );
    assert_eq!(info["profile"], "core");
    assert_eq!(info["zstd_level"], 0);
    assert_eq!(info["target_chunk_size"], 4 * 1024 * 1024);
    assert_eq!(info["max_chunk_size"], 16 * 1024 * 1024);
    assert_eq!(info["dense_line_index"], false);
    assert!(info["original_size"].is_u64());
    assert!(info["document_count"].is_u64());
    assert_keys(&info["original_checksum"], &["algorithm", "value"]);

    let verify = run_json(&["verify", packed, "--format", "json"]);
    assert_keys(&verify, &["checked_chunks", "decoded_bytes", "level", "ok"]);
    assert_eq!(verify["level"], "normal");
    assert_eq!(verify["ok"], true);
    assert_eq!(verify["decoded_bytes"], 0);

    let search = run_json(&["search", packed, "alpha", "--format", "json"]);
    assert_keys(&search, &["capped", "hits", "incomplete_reason", "metrics"]);
    assert!(search["hits"].is_array());
    assert_eq!(search["metrics"]["index_kind"], "token");
    assert_keys(
        &search["metrics"],
        &[
            "candidate_chunks",
            "candidate_granules",
            "decoded_bytes",
            "index_kind",
            "index_size_bytes",
            "index_size_ratio",
            "physical_decoded_bytes",
            "posting_bytes_read",
            "posting_granularity",
            "query",
            "query_time_ms",
            "source_size_bytes",
            "term_lookups",
            "verified_matches",
        ],
    );
    let hit = &search["hits"][0];
    assert_keys(
        hit,
        &[
            "byte_length",
            "chunk_end",
            "chunk_start",
            "logical_offset",
            "source",
        ],
    );

    let docs = run_json(&["docs", documents, "--format", "json"]);
    assert_keys(&docs, &["documents"]);
    let document = &docs["documents"][0];
    assert_keys(
        document,
        &[
            "byte_length",
            "checksum",
            "doc_id",
            "first_line",
            "line_count",
            "logical_offset",
        ],
    );
    assert_keys(&document["checksum"], &["algorithm", "value"]);

    let attest = run_json(&["attest", packed]);
    assert_keys(
        &attest,
        &[
            "chunk_count",
            "container_checksum",
            "container_id",
            "final_file_size",
            "format",
            "line_count",
            "original_checksum",
            "original_size",
            "verify",
        ],
    );
    assert_eq!(attest["verify"]["level"], "deep");
    assert_keys(
        &attest["verify"],
        &["checked_chunks", "decoded_bytes", "level"],
    );

    assert_eq!(SearchOptions::default().max_candidate_granules, 10_000);
    assert_eq!(
        SearchOptions::default().max_decoded_bytes,
        256 * 1024 * 1024
    );
    assert_eq!(SearchOptions::default().max_search_results, u64::MAX);
}

fn assert_usage_error(arguments: &[&str], expected_stderr: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(arguments)
        .output()
        .expect("qzt command should run");
    assert_eq!(output.status.code(), Some(2), "arguments: {arguments:?}");
    assert!(output.stdout.is_empty(), "arguments: {arguments:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected_stderr),
        "arguments: {arguments:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_json(arguments: &[&str]) -> serde_json::Value {
    let Output {
        status,
        stdout,
        stderr,
    } = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(arguments)
        .output()
        .expect("qzt command should run");
    assert!(
        status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&stderr)
    );
    assert!(stderr.is_empty());
    serde_json::from_slice(&stdout).expect("stdout should be valid JSON")
}

fn assert_keys(value: &serde_json::Value, expected: &[&str]) {
    let actual = value
        .as_object()
        .expect("value should be a JSON object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected.iter().copied().collect::<BTreeSet<_>>());
}

struct CliFixture {
    base: std::path::PathBuf,
    packed: std::path::PathBuf,
    documents: std::path::PathBuf,
}

impl CliFixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!("qzt-phase41-{}", std::process::id()));
        fs::create_dir_all(&base).expect("create fixture directory");
        let packed = base.join("plain.qzt");
        let documents = base.join("documents.qzt");
        let input = b"alpha\nbeta\n";
        fs::write(
            &packed,
            pack_bytes(input, WriterOptions::default()).expect("pack plain fixture"),
        )
        .expect("write plain fixture");
        fs::write(
            &documents,
            WriterBuilder::new()
                .document_spans(vec![DocumentSpan::new("doc-1", 0, input.len() as u64)])
                .pack(input)
                .expect("pack document fixture"),
        )
        .expect("write document fixture");
        Self {
            base,
            packed,
            documents,
        }
    }
}

impl Drop for CliFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
