//! Development CLI replay against #325's published-pre.5 evidence container.
//! Original logs are an independent test oracle, never a context input.

use std::fmt::Write as _;
use std::fs;
use std::process::Command;

use serde_json::Value;

const PRE4: &[u8] = include_bytes!("../examples/ci-log-handoff/inputs/pre4-linux.log");
const PRE5: &[u8] = include_bytes!("../examples/ci-log-handoff/inputs/pre5-linux.log");
const QZT: &[u8] = include_bytes!("../examples/ci-log-handoff/evidence.qzt");

fn search(path: &str, query: &str) -> Vec<(usize, usize)> {
    let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args([
            "search", path, query, "--index", "ngram", "--ngram", "3", "--format", "json",
        ])
        .output()
        .expect("search");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["capped"], false);
    value["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|hit| {
            (
                usize::try_from(hit["logical_offset"].as_u64().unwrap()).unwrap(),
                usize::try_from(hit["byte_length"].as_u64().unwrap()).unwrap(),
            )
        })
        .collect()
}

fn oracle_hits(query: &[u8]) -> Vec<(usize, usize)> {
    let joined = [PRE4, PRE5].concat();
    joined
        .windows(query.len())
        .enumerate()
        .filter_map(|(start, bytes)| (bytes == query).then_some((start, query.len())))
        .collect()
}

#[test]
fn qzt_only_recipient_can_follow_search_hits_to_document_and_original_lines() {
    let recipient = tempfile::tempdir().unwrap();
    let path = recipient.path().join("evidence.qzt");
    fs::write(&path, QZT).unwrap();
    let path = path.to_str().unwrap();
    assert_eq!(fs::read_dir(recipient.path()).unwrap().count(), 1);

    for (query, expected_count) in [
        ("RuntimeError: build checkout is dirty", 1),
        ("build environment unchanged:", 1),
        ("dist ran successfully", 4),
        ("Traceback (most recent call last):", 1), // not in #325's three-query manifest
    ] {
        let hits = search(path, query);
        assert_eq!(hits, oracle_hits(query.as_bytes()), "{query}");
        assert_eq!(hits.len(), expected_count, "{query}");
        for (global, length) in hits {
            let (id, source, local) = if global < PRE4.len() {
                ("ci/pre4-linux.log", PRE4, global)
            } else {
                ("ci/pre5-linux.log", PRE5, global - PRE4.len())
            };
            let offset = global.to_string();
            let length_arg = length.to_string();
            let before = if query.starts_with("RuntimeError:") {
                "12"
            } else {
                "2"
            };
            let output = Command::new(env!("CARGO_BIN_EXE_qzt"))
                .args([
                    "context",
                    path,
                    "--offset",
                    &offset,
                    "--length",
                    &length_arg,
                    "--before",
                    before,
                    "--after",
                    "2",
                    "--format",
                    "json",
                ])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let result: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(result["mapping_status"], "unique");
            assert_eq!(result["document"]["id"], id);
            assert_eq!(result["document"]["local_offset"], local);
            assert_eq!(result["document"]["local_end"], local + length);
            let excerpt_global =
                usize::try_from(result["excerpt"]["logical_offset"].as_u64().unwrap()).unwrap();
            let excerpt_len =
                usize::try_from(result["excerpt"]["byte_length"].as_u64().unwrap()).unwrap();
            let doc_start = global - local;
            assert!(excerpt_global >= doc_start);
            assert!(excerpt_global + excerpt_len <= doc_start + source.len());
            let expected =
                &source[excerpt_global - doc_start..excerpt_global - doc_start + excerpt_len];
            assert_eq!(result["excerpt"]["bytes_hex"], qzt_hex(expected));
            assert_eq!(&source[local..local + length], query.as_bytes());
            let display = result["excerpt"]["text_escaped"].as_str().unwrap();
            if query.starts_with("RuntimeError:") {
                assert!(display.contains("Traceback (most recent call last):"));
                assert!(display.contains("snapshot"));
                assert!(display.contains("exit code 1"));
            }
            if query == "dist ran successfully" {
                assert_eq!(
                    display.contains("echo \"dist ran successfully\""),
                    local < 27_000
                );
            }
        }
    }
    assert_eq!(fs::read_dir(recipient.path()).unwrap().count(), 1);
}

fn qzt_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}
