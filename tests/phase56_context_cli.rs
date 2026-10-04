use std::fs;
use std::process::{Command, Output};

use qzt::{DocumentSpan, WriterBuilder, WriterOptions, pack_bytes};
use serde_json::Value;

fn fixture(bytes: &[u8], spans: &[(&str, u64, u64)]) -> (tempfile::TempDir, String) {
    let documents = spans
        .iter()
        .map(|(id, start, length)| DocumentSpan::new(*id, *start, *length))
        .collect();
    let packed = WriterBuilder::new()
        .document_spans(documents)
        .pack(bytes)
        .expect("pack fixture");
    let dir = tempfile::tempdir().expect("temp directory");
    let path = dir.path().join("fixture.qzt");
    fs::write(&path, packed).expect("fixture write");
    (dir, path.to_str().unwrap().to_owned())
}

fn run(path: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qzt"))
        .arg("context")
        .arg(path)
        .args(args)
        .output()
        .expect("run context")
}

fn json(path: &str, args: &[&str]) -> Value {
    let output = run(path, args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON")
}

#[test]
fn unique_document_maps_hit_and_restores_exact_lines() {
    let original = b"alpha\r\nbeta\nlast";
    let (_dir, path) = fixture(original, &[("log", 0, original.len() as u64)]);
    let value = json(
        &path,
        &[
            "--offset", "7", "--length", "4", "--before", "1", "--after", "1", "--format", "json",
        ],
    );
    assert_eq!(value["mapping_status"], "unique");
    assert_eq!(value["document"]["id"], "log");
    assert_eq!(value["document"]["local_offset"], 7);
    assert_eq!(value["document"]["local_end"], 11);
    assert_eq!(value["excerpt"]["logical_offset"], 0);
    assert_eq!(value["excerpt"]["byte_length"], original.len());
    assert_eq!(
        value["excerpt"]["bytes_hex"],
        "616c7068610d0a626574610a6c617374"
    );
    assert_eq!(value["before"]["stop"], "complete");
    assert_eq!(value["after"]["stop"], "complete");
    assert_eq!(value["verification"]["document_checksum_verified"], false);
}

#[test]
fn empty_document_inside_hit_does_not_change_unique_mapping_or_scope() {
    let original = b"prefix\nabcdef\nsuffix\n";
    let (_dir, path) = fixture(original, &[("body", 7, 7), ("empty", 10, 0)]);
    let args = [
        "--offset", "9", "--length", "2", "--before", "2", "--after", "2",
    ];
    let value = json(&path, &[&args[..], &["--format", "json"]].concat());
    assert_eq!(value["mapping_status"], "unique");
    assert_eq!(value["document"]["id"], "body");
    assert_eq!(value["document"]["local_offset"], 2);
    assert_eq!(value["document"]["local_end"], 4);
    assert_eq!(value["scope"]["kind"], "document");
    assert_eq!(value["scope"]["logical_offset"], 7);
    assert_eq!(value["scope"]["end"], 14);
    assert_eq!(value["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(value["candidates"][0]["id"], "body");
    let start = usize::try_from(value["excerpt"]["logical_offset"].as_u64().unwrap()).unwrap();
    let length = usize::try_from(value["excerpt"]["byte_length"].as_u64().unwrap()).unwrap();
    assert_eq!((start, start + length), (7, 14));
    let restored: Vec<u8> = value["excerpt"]["bytes_hex"]
        .as_str()
        .unwrap()
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(restored, original[start..start + length]);
    assert_eq!(&restored[9 - start..11 - start], b"cd");
    let text = String::from_utf8(run(&path, &args).stdout).unwrap();
    assert!(text.contains("mapping: unique"));
    assert!(text.contains("scope: document 7:14"));
    assert!(text.contains("document: body local 2:4"));
    assert!(text.contains("candidate: body 7:14"));
    assert!(!text.contains("candidate: empty"));
    assert!(text.contains("excerpt 7:14"));
    for direction in ["before", "after"] {
        let detail = &value[direction];
        assert_eq!(detail["stop"], "scope_boundary");
        assert!(text.contains(&format!(
            "{direction}: {}/{} stop={}",
            detail["returned"].as_u64().unwrap(),
            detail["requested"].as_u64().unwrap(),
            detail["stop"].as_str().unwrap(),
        )));
    }
}

#[test]
fn empty_documents_at_hit_edges_and_outside_are_not_candidates() {
    for empty_offset in [9, 10, 11, 20] {
        let original = b"prefix\nabcdef\nsuffix\n";
        let (_dir, path) = fixture(original, &[("body", 7, 7), ("empty", empty_offset, 0)]);
        let value = json(
            &path,
            &["--offset", "9", "--length", "2", "--format", "json"],
        );
        assert_eq!(value["mapping_status"], "unique", "empty at {empty_offset}");
        assert_eq!(value["candidates"].as_array().unwrap().len(), 1);
        assert_eq!(value["candidates"][0]["id"], "body");
    }
}

#[test]
fn empty_only_document_index_leaves_positive_hit_unmapped() {
    let (_dir, path) = fixture(b"abc\n", &[("empty", 1, 0)]);
    let value = json(
        &path,
        &["--offset", "0", "--length", "3", "--format", "json"],
    );
    assert_eq!(value["mapping_status"], "unmapped");
    assert!(value["document"].is_null());
    assert_eq!(value["scope"]["kind"], "container");
    assert_eq!(
        value["candidates"].as_array().unwrap().as_slice(),
        [] as [serde_json::Value; 0]
    );
}

#[test]
fn empty_entries_count_for_document_budget_but_not_candidate_budget() {
    let original = b"prefix\nabcdef\nsuffix\n";
    let names: Vec<_> = (0..256).map(|i| format!("empty-{i}")).collect();
    let mut spans = vec![("body", 7, 7)];
    spans.extend(names.iter().map(|name| (name.as_str(), 10, 0)));
    let (_dir, path) = fixture(original, &spans);
    let args = ["--offset", "9", "--length", "2", "--format", "json"];
    let value = json(&path, &[&args[..], &["--max-documents", "257"]].concat());
    assert_eq!(value["mapping_status"], "unique");
    assert_eq!(value["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(
        run(&path, &[&args[..], &["--max-documents", "256"]].concat())
            .status
            .code(),
        Some(1)
    );
}

#[test]
fn overlap_gap_crossing_and_empty_documents_never_invent_local_positions() {
    let original = b"alpha beta\n";
    let (_dir, path) = fixture(
        original,
        &[("a", 0, 6), ("b", 0, 6), ("empty", 6, 0), ("c", 6, 4)],
    );
    let ambiguous = json(
        &path,
        &["--offset", "1", "--length", "2", "--format", "json"],
    );
    assert_eq!(ambiguous["mapping_status"], "ambiguous");
    assert!(ambiguous["document"].is_null());
    assert_eq!(ambiguous["candidates"].as_array().unwrap().len(), 2);
    assert_eq!(ambiguous["candidates"][0]["id"], "a");
    assert_eq!(ambiguous["candidates"][1]["id"], "b");
    let crossing = json(
        &path,
        &["--offset", "5", "--length", "2", "--format", "json"],
    );
    assert_eq!(crossing["mapping_status"], "cross_document");
    assert!(crossing["document"].is_null());
    assert_eq!(crossing["candidates"].as_array().unwrap().len(), 3);
    assert_eq!(crossing["candidates"][2]["id"], "c");
    let (_baseline_dir, baseline_path) =
        fixture(original, &[("a", 0, 6), ("b", 0, 6), ("c", 6, 4)]);
    for (offset, length, with_empty) in [(1, 2, &ambiguous), (5, 2, &crossing)] {
        let baseline = json(
            &baseline_path,
            &[
                "--offset",
                &offset.to_string(),
                "--length",
                &length.to_string(),
                "--format",
                "json",
            ],
        );
        assert_eq!(with_empty["mapping_status"], baseline["mapping_status"]);
        assert_eq!(with_empty["candidates"], baseline["candidates"]);
    }
    let gap = json(
        &path,
        &["--offset", "10", "--length", "1", "--format", "json"],
    );
    assert_eq!(gap["mapping_status"], "unmapped");
    assert!(gap["document"].is_null());
}

#[test]
fn missing_index_and_argument_errors_are_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plain.qzt");
    fs::write(
        &path,
        pack_bytes(b"one\ntwo", WriterOptions::default()).unwrap(),
    )
    .unwrap();
    let path = path.to_str().unwrap();
    let value = json(
        path,
        &["--offset", "4", "--length", "3", "--format", "json"],
    );
    assert_eq!(value["mapping_status"], "no_document_index");
    assert!(value["document"].is_null());
    for args in [
        vec!["--offset", "4", "--length", "0"],
        vec!["--offset", "18446744073709551615", "--length", "2"],
        vec!["--offset", "x", "--length", "2"],
    ] {
        assert_eq!(run(path, &args).status.code(), Some(2));
    }
    assert_eq!(
        run(path, &["--offset", "6", "--length", "2"]).status.code(),
        Some(1)
    );
}

#[test]
fn insufficient_context_budget_is_distinct_from_missing_hit() {
    let original = b"start\nneedle\nfinish\n";
    let (_dir, path) = fixture(original, &[("log", 0, original.len() as u64)]);
    let clipped = json(
        &path,
        &[
            "--offset",
            "6",
            "--length",
            "6",
            "--before",
            "1",
            "--after",
            "1",
            "--max-scan-bytes",
            "6",
            "--format",
            "json",
        ],
    );
    assert_eq!(clipped["excerpt"]["bytes_hex"], "6e6565646c65");
    assert_eq!(clipped["before"]["stop"], "budget");
    assert_eq!(clipped["after"]["stop"], "budget");
    assert_eq!(
        run(
            &path,
            &["--offset", "6", "--length", "6", "--max-scan-bytes", "5"]
        )
        .status
        .code(),
        Some(1)
    );
}

#[test]
fn document_boundary_can_split_a_physical_line_without_claiming_query_scope() {
    let original = b"alpha beta\n";
    let (_dir, path) = fixture(original, &[("a", 0, 6), ("b", 6, 5)]);
    let value = json(
        &path,
        &[
            "--offset", "6", "--length", "4", "--before", "2", "--after", "2", "--format", "json",
        ],
    );
    assert_eq!(value["mapping_status"], "unique");
    assert_eq!(value["document"]["id"], "b");
    assert_eq!(value["scope"]["logical_offset"], 6);
    assert_eq!(value["excerpt"]["bytes_hex"], "626574610a");
    assert_eq!(value["before"]["stop"], "scope_boundary");
    assert_eq!(value["verification"]["search_query_verified"], false);
}

#[test]
fn raw_bytes_survive_utf8_cuts_and_text_escapes_controls() {
    let original = "日\u{1b}[31m本\r\n".as_bytes();
    let (_dir, path) = fixture(original, &[("jp", 0, original.len() as u64)]);
    let value = json(
        &path,
        &[
            "--offset",
            "1",
            "--length",
            "1",
            "--max-scan-bytes",
            "1",
            "--format",
            "json",
        ],
    );
    assert_eq!(value["excerpt"]["bytes_hex"], "97");
    assert_eq!(value["excerpt"]["text_escaped"], "\\x97");
    assert_eq!(value["before"]["stop"], "budget");
    let text = run(&path, &["--offset", "3", "--length", "5"]).stdout;
    assert!(!text.contains(&0x1b));
    assert!(String::from_utf8(text).unwrap().contains("\\x1b"));
}

#[test]
fn physical_chunk_and_render_budgets_gate_the_whole_command() {
    let original = b"one\ntwo\n";
    let (_dir, path) = fixture(original, &[("log", 0, original.len() as u64)]);
    let args = ["--offset", "4", "--length", "3", "--format", "json"];
    let output = run(&path, &args);
    assert!(output.status.success());
    let rendered = output.stdout.len().to_string();
    assert!(
        run(
            &path,
            &[
                "--offset",
                "4",
                "--length",
                "3",
                "--format",
                "json",
                "--max-output-bytes",
                &rendered,
            ]
        )
        .status
        .success()
    );
    let short = (output.stdout.len() - 1).to_string();
    assert_eq!(
        run(
            &path,
            &[
                "--offset",
                "4",
                "--length",
                "3",
                "--format",
                "json",
                "--max-output-bytes",
                &short,
            ]
        )
        .status
        .code(),
        Some(1)
    );
    assert!(
        run(
            &path,
            &[
                "--offset",
                "4",
                "--length",
                "3",
                "--max-physical-decoded-bytes",
                "8",
            ]
        )
        .status
        .success()
    );
    assert_eq!(
        run(
            &path,
            &[
                "--offset",
                "4",
                "--length",
                "3",
                "--max-physical-decoded-bytes",
                "7",
            ]
        )
        .status
        .code(),
        Some(1)
    );
    assert_eq!(
        run(
            &path,
            &[
                "--offset",
                "4",
                "--length",
                "3",
                "--max-physical-decoded-chunks",
                "0",
            ]
        )
        .status
        .code(),
        Some(1)
    );
    assert_eq!(
        run(
            &path,
            &["--offset", "4", "--length", "3", "--max-documents", "0",]
        )
        .status
        .code(),
        Some(1)
    );
}

#[test]
fn corrupt_chunk_is_error_and_dli_does_not_change_context() {
    let original = b"first-line-continues\nsecond\n";
    let dir = tempfile::tempdir().unwrap();
    let mut outputs = Vec::new();
    for dli in [false, true] {
        let packed = WriterBuilder::new()
            .options(qzt::WriterOptions {
                chunker: qzt::ChunkerOptions {
                    target_chunk_size: 8,
                    max_chunk_size: 8,
                },
                zstd_level: 0,
            })
            .dense_line_index(dli)
            .pack(original)
            .unwrap();
        let path = dir.path().join(format!("dli-{dli}.qzt"));
        fs::write(&path, &packed).unwrap();
        let path_str = path.to_str().unwrap();
        let value = json(
            path_str,
            &[
                "--offset", "11", "--length", "4", "--before", "0", "--after", "1", "--format",
                "json",
            ],
        );
        outputs.push(value["excerpt"]["bytes_hex"].clone());
        if !dli {
            let reader = qzt::QztFileReader::open_path(&path).unwrap();
            let entry = &reader.skeleton_details().chunk_entries[1];
            let mut broken = packed;
            broken[usize::try_from(entry.physical_offset).unwrap()] ^= 1;
            fs::write(&path, broken).unwrap();
            assert_eq!(
                run(path_str, &["--offset", "11", "--length", "4",])
                    .status
                    .code(),
                Some(1)
            );
        }
    }
    assert_eq!(outputs[0], outputs[1]);
}

#[test]
fn multiline_hit_counts_lines_outside_both_hit_endpoints() {
    let original = b"a\nb\nc\nd\n";
    let (_dir, path) = fixture(original, &[("log", 0, original.len() as u64)]);
    let value = json(
        &path,
        &[
            "--offset", "2", "--length", "3", "--before", "1", "--after", "1", "--format", "json",
        ],
    );
    assert_eq!(value["excerpt"]["bytes_hex"], "610a620a630a640a");
    assert_eq!(value["before"]["returned"], 1);
    assert_eq!(value["after"]["returned"], 1);
}

#[test]
fn partial_preceding_line_is_budget_stop_even_when_one_segment_is_visible() {
    let mut original = vec![b'a'; 100_000];
    original.extend_from_slice(b"\nneedle\n");
    let (_dir, path) = fixture(&original, &[("log", 0, original.len() as u64)]);
    let value = json(
        &path,
        &[
            "--offset",
            "100001",
            "--length",
            "6",
            "--before",
            "1",
            "--max-scan-bytes",
            "20",
            "--format",
            "json",
        ],
    );
    assert_eq!(value["before"]["returned"], 1);
    assert_eq!(value["before"]["stop"], "budget");
    assert_eq!(value["excerpt"]["leading_fragment"], true);
}

#[test]
fn scan_ending_just_after_lf_keeps_the_complete_hit_line() {
    let original = b"h\nx\ny\n";
    let (_dir, path) = fixture(original, &[("log", 0, original.len() as u64)]);
    let value = json(
        &path,
        &[
            "--offset",
            "2",
            "--length",
            "1",
            "--before",
            "0",
            "--after",
            "0",
            "--max-scan-bytes",
            "3",
            "--format",
            "json",
        ],
    );
    assert_eq!(value["excerpt"]["bytes_hex"], "780a");
    assert_eq!(value["excerpt"]["trailing_fragment"], false);
    assert_eq!(value["after"]["stop"], "complete");
    let wants_next = json(
        &path,
        &[
            "--offset",
            "2",
            "--length",
            "1",
            "--before",
            "0",
            "--after",
            "1",
            "--max-scan-bytes",
            "3",
            "--format",
            "json",
        ],
    );
    assert_eq!(wants_next["after"]["stop"], "budget");
}

#[test]
fn scan_starting_just_after_lf_keeps_the_complete_preceding_line() {
    let original = b"aa\nbb\ncc";
    let (_dir, path) = fixture(original, &[("log", 0, original.len() as u64)]);
    let value = json(
        &path,
        &[
            "--offset",
            "6",
            "--length",
            "2",
            "--before",
            "1",
            "--after",
            "0",
            "--max-scan-bytes",
            "8",
            "--format",
            "json",
        ],
    );
    assert_eq!(value["excerpt"]["bytes_hex"], "62620a6363");
    assert_eq!(value["excerpt"]["leading_fragment"], false);
    assert_eq!(value["before"]["stop"], "complete");
}

#[test]
fn bidi_and_invisible_format_controls_are_escaped_in_display_only() {
    let original = "x\u{2066}y\n".as_bytes();
    let (_dir, path) = fixture(original, &[("lo\u{202e}g", 0, original.len() as u64)]);
    let value = json(
        &path,
        &["--offset", "1", "--length", "3", "--format", "json"],
    );
    assert_eq!(value["excerpt"]["bytes_hex"], "78e281a6790a");
    assert!(
        value["excerpt"]["text_escaped"]
            .as_str()
            .unwrap()
            .contains("\\u{2066}")
    );
    let text = String::from_utf8(run(&path, &["--offset", "1", "--length", "3"]).stdout).unwrap();
    assert!(text.contains("\\u{202e}"));
    assert!(text.contains("\\u{2066}"));
    assert!(!text.contains('\u{202e}'));
    assert!(!text.contains('\u{2066}'));
}

#[test]
fn hit_at_document_edges_does_not_escape_its_scope() {
    let original = b"head\ntail";
    let (_dir, path) = fixture(original, &[("head", 0, 5), ("tail", 5, 4)]);
    let first = json(
        &path,
        &[
            "--offset", "0", "--length", "1", "--before", "2", "--format", "json",
        ],
    );
    assert_eq!(first["document"]["id"], "head");
    assert_eq!(first["excerpt"]["bytes_hex"], "686561640a");
    assert_eq!(first["before"]["stop"], "scope_boundary");
    let last = json(
        &path,
        &[
            "--offset", "8", "--length", "1", "--after", "2", "--format", "json",
        ],
    );
    assert_eq!(last["document"]["id"], "tail");
    assert_eq!(last["excerpt"]["bytes_hex"], "7461696c");
    assert_eq!(last["after"]["stop"], "scope_boundary");
}

#[test]
fn text_and_json_agree_on_coordinates_status_stops_and_display() {
    let original = b"one\n\x1b[31mtwo\nthree";
    let (_dir, path) = fixture(original, &[("log", 0, original.len() as u64)]);
    let args = [
        "--offset", "9", "--length", "3", "--before", "1", "--after", "1",
    ];
    for extra in [&[][..], &["--max-scan-bytes", "3"][..]] {
        let args = [&args[..], extra].concat();
        let value = json(&path, &[&args[..], &["--format", "json"]].concat());
        let text = String::from_utf8(run(&path, &args).stdout).unwrap();
        let hit_start = value["hit"]["logical_offset"].as_u64().unwrap();
        let hit_end = value["hit"]["end"].as_u64().unwrap();
        let excerpt_start = value["excerpt"]["logical_offset"].as_u64().unwrap();
        let excerpt_end = excerpt_start + value["excerpt"]["byte_length"].as_u64().unwrap();
        assert!(text.contains(&format!("hit {hit_start}:{hit_end}")));
        assert!(text.contains("mapping: unique"));
        assert!(text.contains(&format!("excerpt {excerpt_start}:{excerpt_end}")));
        for direction in ["before", "after"] {
            let detail = &value[direction];
            assert!(text.contains(&format!(
                "{direction}: {}/{} stop={}",
                detail["returned"].as_u64().unwrap(),
                detail["requested"].as_u64().unwrap(),
                detail["stop"].as_str().unwrap(),
            )));
        }
        assert!(text.contains(value["excerpt"]["text_escaped"].as_str().unwrap()));
        assert!(!text.contains('\x1b'));
        for edge in ["leading_fragment", "trailing_fragment"] {
            assert!(text.contains(&format!(
                "{edge}={}",
                value["excerpt"][edge].as_bool().unwrap()
            )));
        }
        if !extra.is_empty() {
            assert_eq!(value["before"]["stop"], "budget");
            assert_eq!(value["after"]["stop"], "budget");
            assert_eq!(value["excerpt"]["bytes_hex"], "74776f");
        }
    }
}

#[test]
fn a_partially_overlapping_second_document_makes_mapping_ambiguous() {
    let original = b"abcdefgh\n";
    let (_dir, path) = fixture(
        original,
        &[("full", 0, 8), ("partial", 4, 4), ("empty", 3, 0)],
    );
    let value = json(
        &path,
        &["--offset", "2", "--length", "4", "--format", "json"],
    );
    assert_eq!(value["mapping_status"], "ambiguous");
    assert!(value["document"].is_null());
    assert_eq!(value["candidates"].as_array().unwrap().len(), 2);
}

#[cfg(feature = "internal-testing")]
#[test]
fn malformed_document_range_is_error_but_stale_document_checksum_is_not_claimed() {
    use qzt::{Checksum, DocumentEntry, DocumentIndex};

    let dir = tempfile::tempdir().unwrap();
    let source = b"valid\n";
    let container_id = [0x57; 16];
    for (name, entry, expected_status) in [
        (
            "bad-range",
            DocumentEntry::new("bad", 7, 1, 0, 1, 0, 1, Checksum::blake3(b"")),
            1,
        ),
        (
            "stale-checksum",
            DocumentEntry::new("stale", 0, 6, 0, 1, 0, 1, Checksum::blake3(b"other")),
            0,
        ),
        (
            "bad-empty-range",
            DocumentEntry::new("bad-empty", 7, 0, 0, 1, 0, 1, Checksum::blake3(b"")),
            1,
        ),
    ] {
        let packed = qzt::writer::pack_bytes_with_document_index_override(
            source,
            container_id,
            WriterOptions::default(),
            &DocumentIndex {
                container_id,
                documents: vec![entry],
            },
        )
        .unwrap();
        let path = dir.path().join(format!("{name}.qzt"));
        fs::write(&path, packed).unwrap();
        let output = run(
            path.to_str().unwrap(),
            &["--offset", "0", "--length", "5", "--format", "json"],
        );
        assert_eq!(output.status.code(), Some(expected_status), "{name}");
        if expected_status == 0 {
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(value["verification"]["document_checksum_verified"], false);
        }
    }
}
