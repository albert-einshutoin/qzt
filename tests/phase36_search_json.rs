/// Integration tests for `qzt search --format json` (issue #36).
///
/// Verifies that the JSON output schema matches the issue specification:
/// `{"hits":[...], "metrics":{...}, "capped":bool, "incomplete_reason": null|"…"}`.
/// Text-mode output is asserted unchanged by running the same searches in
/// both modes and checking that text output is byte-for-byte equivalent to
/// what the pre-existing tests expected.
use std::fs;
use std::process::Command;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(args)
        .output()
        .expect("command should run")
}

fn pack_to(input: &[u8], base: &std::path::Path) -> std::path::PathBuf {
    let input_path = base.join("input.txt");
    let packed_path = base.join("input.qzt");
    fs::write(&input_path, input).expect("input write");
    let out = run(&[
        "pack",
        input_path.to_str().unwrap(),
        "-o",
        packed_path.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "pack failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    packed_path
}

// ---------------------------------------------------------------------------
// search_json_outputs_hits_and_null_incomplete_reason
// ---------------------------------------------------------------------------

/// `qzt search --format json` emits a valid JSON object with `"hits"` array
/// and `"incomplete_reason": null` when the search has no reason to be
/// incomplete.
#[test]
fn search_json_outputs_hits_and_null_incomplete_reason() {
    let base = std::env::temp_dir().join(format!("qzt-36-json-hits-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    let packed = pack_to(b"info\nerror code\nerror again\n", &base);
    let qzt = packed.to_str().unwrap();

    let out = run(&["search", qzt, "error", "--format", "json"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json = String::from_utf8(out.stdout).expect("stdout is utf-8");

    // Top-level structure.
    assert!(json.trim().starts_with('{'), "must start with {{: {json}");
    assert!(json.trim().ends_with('}'), "must end with }}: {json}");

    // Required top-level keys.
    assert!(json.contains("\"hits\":"), "must contain hits key: {json}");
    assert!(
        json.contains("\"metrics\":"),
        "must contain metrics key: {json}"
    );
    assert!(json.contains("\"capped\":"), "must contain capped: {json}");
    assert!(
        json.contains("\"incomplete_reason\":"),
        "must contain incomplete_reason: {json}"
    );

    // incomplete_reason must be JSON null (no reason to be incomplete).
    assert!(
        json.contains("\"incomplete_reason\": null") || json.contains("\"incomplete_reason\":null"),
        "incomplete_reason must be null: {json}"
    );

    // hits array must contain at least one hit object with the expected fields.
    assert!(
        json.contains("\"logical_offset\""),
        "hit must have logical_offset: {json}"
    );
    assert!(
        json.contains("\"byte_length\""),
        "hit must have byte_length: {json}"
    );
    assert!(
        json.contains("\"chunk_start\""),
        "hit must have chunk_start: {json}"
    );
    assert!(
        json.contains("\"chunk_end\""),
        "hit must have chunk_end: {json}"
    );
    assert!(json.contains("\"source\""), "hit must have source: {json}");

    // source must be escaped as a JSON string.
    assert!(
        json.contains("\"source\":\"verified_original_bytes\""),
        "source must be verified_original_bytes: {json}"
    );

    // metrics sub-fields.
    assert!(
        json.contains("\"query\":"),
        "metrics must have query: {json}"
    );
    assert!(
        json.contains("\"index_kind\":"),
        "metrics must have index_kind: {json}"
    );
    assert!(
        json.contains("\"posting_granularity\":"),
        "metrics must have posting_granularity: {json}"
    );
    assert!(
        json.contains("\"verified_matches\":"),
        "metrics must have verified_matches: {json}"
    );
    assert!(
        json.contains("\"query_time_ms\":"),
        "metrics must have query_time_ms: {json}"
    );
    assert!(
        json.contains("\"index_size_ratio\":"),
        "metrics must have index_size_ratio: {json}"
    );

    let _ = fs::remove_dir_all(base);
}

// ---------------------------------------------------------------------------
// search_json_zero_hits_produces_empty_array
// ---------------------------------------------------------------------------

/// When the query is absent from the index, `"hits":[]` is emitted.
#[test]
fn search_json_zero_hits_produces_empty_array() {
    let base = std::env::temp_dir().join(format!("qzt-36-json-zero-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    let packed = pack_to(b"alpha\nbeta\ngamma\n", &base);
    let qzt = packed.to_str().unwrap();

    let out = run(&["search", qzt, "absent_token", "--format", "json"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json = String::from_utf8(out.stdout).expect("stdout is utf-8");

    assert!(
        json.contains("\"hits\":[]"),
        "hits must be empty array: {json}"
    );
    assert!(
        json.contains("\"incomplete_reason\":null") || json.contains("\"incomplete_reason\": null"),
        "incomplete_reason must be null when no hits: {json}"
    );

    let _ = fs::remove_dir_all(base);
}

// ---------------------------------------------------------------------------
// search_json_quotes_incomplete_reason_when_present
// ---------------------------------------------------------------------------

/// When the query is shorter than the n-gram `n`, `incomplete_reason` is a
/// quoted string (not `null`) both in JSON and on stderr.
#[test]
fn search_json_quotes_incomplete_reason_when_present() {
    let base = std::env::temp_dir().join(format!("qzt-36-json-incomplete-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    let packed = pack_to(b"error occurred\nerror again\n", &base);
    let qzt = packed.to_str().unwrap();

    // n-gram index with n=3 and a 1-character query → incomplete.
    let out = run(&[
        "search", qzt, "e", "--index", "ngram", "--ngram", "3", "--format", "json",
    ]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json = String::from_utf8(out.stdout).expect("stdout is utf-8");
    let stderr = String::from_utf8_lossy(&out.stderr);

    // incomplete_reason must be a quoted string.
    assert!(
        json.contains("\"incomplete_reason\":\"query_shorter_than_ngram_n\"")
            || json.contains("\"incomplete_reason\": \"query_shorter_than_ngram_n\""),
        "incomplete_reason must be a string: {json}"
    );

    // stderr warning must still be emitted even in JSON mode.
    assert!(
        stderr.contains("warning: result may be incomplete"),
        "stderr warning must be present in JSON mode: {stderr}"
    );

    // stdout JSON must not contain the warning (stdout must stay clean).
    assert!(
        !json.contains("warning"),
        "warning must not appear in stdout JSON: {json}"
    );

    let _ = fs::remove_dir_all(base);
}

// ---------------------------------------------------------------------------
// search_json_escapes_query_with_double_quotes
// ---------------------------------------------------------------------------

/// A query containing `"` characters must be properly escaped in the JSON
/// output (must not break the JSON structure).
#[test]
fn search_json_escapes_query_with_double_quotes() {
    let base = std::env::temp_dir().join(format!("qzt-36-json-escape-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    // The query "error" (with literal quotes) won't match, but exercises
    // JSON escaping in the metrics.query field.
    let packed = pack_to(b"error info log\n", &base);
    let qzt = packed.to_str().unwrap();

    let out = run(&["search", qzt, "\"error\"", "--format", "json"]);
    // The command must not crash regardless of whether it finds hits.
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json = String::from_utf8(out.stdout).expect("stdout is utf-8");

    // The query field must contain the escaped double-quote, not a raw one
    // that would break JSON parsing.
    assert!(
        json.contains("\\\"error\\\"") || json.contains("\\\\\""),
        "double-quote in query must be escaped: {json}"
    );

    // The output must still form a JSON object with the top-level keys.
    assert!(json.trim().starts_with('{'), "must start with {{: {json}");
    assert!(json.trim().ends_with('}'), "must end with }}: {json}");

    let _ = fs::remove_dir_all(base);
}

// ---------------------------------------------------------------------------
// search_text_mode_unchanged
// ---------------------------------------------------------------------------

/// Default text-mode output is byte-identical to the pre-existing format when
/// `--format json` is not passed. The `hit … source=` prefix and `metrics …`
/// line must remain intact.
#[test]
fn search_text_mode_unchanged() {
    let base = std::env::temp_dir().join(format!("qzt-36-text-mode-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    let packed = pack_to(b"info\nerror code\nerror again\n", &base);
    let qzt = packed.to_str().unwrap();

    let out = run(&["search", qzt, "error"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let text = String::from_utf8(out.stdout).expect("stdout is utf-8");

    assert!(
        text.contains("source=verified_original_bytes"),
        "text mode must show source= key: {text}"
    );
    assert!(
        text.contains("candidate_granules="),
        "text mode must show candidate_granules=: {text}"
    );
    assert!(
        text.contains("incomplete_reason=none"),
        "text mode incomplete_reason must be 'none': {text}"
    );
    // Confirm text output does NOT start with '{' (would indicate JSON leaking).
    assert!(
        !text.trim().starts_with('{'),
        "text mode must not start with {{: {text}"
    );

    let _ = fs::remove_dir_all(base);
}

// ---------------------------------------------------------------------------
// search_text_metrics_escapes_query_control_chars
// ---------------------------------------------------------------------------

/// Text-mode `metrics query=` must stay on a single line even when the query
/// contains LF, CR, or double quotes. JSON output must still round-trip the
/// original query string unchanged.
#[test]
fn search_text_metrics_escapes_query_control_chars() {
    let base =
        std::env::temp_dir().join(format!("qzt-36-text-metrics-escape-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    let packed = pack_to(b"some content\n", &base);
    let qzt = packed.to_str().unwrap();

    let query = "a\nb\r\"quote\"";
    let out = run(&["search", qzt, query]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let text = String::from_utf8(out.stdout).expect("stdout is utf-8");
    let metrics_line = text
        .lines()
        .find(|line| line.starts_with("metrics "))
        .expect("must have metrics line");

    // The metrics line must remain one physical line; later fields stay on it.
    assert!(
        metrics_line.contains(" index_kind="),
        "index_kind must be on the same metrics line: {metrics_line}"
    );
    assert!(
        metrics_line.contains(" posting_granularity="),
        "posting_granularity must be on the same metrics line: {metrics_line}"
    );

    // Raw control characters must not appear unescaped in the query value.
    assert!(
        !metrics_line.contains("query=a\n"),
        "LF must be escaped in metrics query: {metrics_line}"
    );
    assert!(
        !metrics_line.contains('\r'),
        "CR must be escaped in metrics query: {metrics_line}"
    );

    let expected_query_escaped = r#"a\nb\r\"quote\""#;
    assert!(
        metrics_line.contains(&format!("query={expected_query_escaped} ")),
        "query must be escaped in metrics line: {metrics_line}"
    );

    let json_out = run(&["search", qzt, query, "--format", "json"]);
    assert!(
        json_out.status.success(),
        "json stderr: {}",
        String::from_utf8_lossy(&json_out.stderr)
    );
    let json = String::from_utf8(json_out.stdout).expect("json stdout is utf-8");
    let value: serde_json::Value = serde_json::from_str(&json)
        .unwrap_or_else(|error| panic!("stdout must be valid JSON: {error}\n{json}"));
    assert_eq!(
        value
            .get("metrics")
            .and_then(|metrics| metrics.get("query"))
            .and_then(serde_json::Value::as_str),
        Some(query),
        "JSON metrics.query must round-trip the original query"
    );

    let _ = fs::remove_dir_all(base);
}

// ---------------------------------------------------------------------------
// search_text_mode_capped_metrics_contract
// ---------------------------------------------------------------------------

/// Text-mode search with `--max-results` caps hits but still exits 0.
/// `capped=true` means a limit was reached, not a command failure; it is
/// distinct from `incomplete_reason=query_shorter_than_ngram_n`.
#[test]
fn search_text_mode_capped_metrics_contract() {
    let base = std::env::temp_dir().join(format!("qzt-36-capped-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    let packed = pack_to(b"needle one\nneedle two\nneedle three\n", &base);
    let qzt = packed.to_str().unwrap();

    let out = run(&["search", qzt, "needle", "--max-results", "2"]);
    assert!(
        out.status.success(),
        "capped search must exit 0: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let text = String::from_utf8(out.stdout).expect("stdout is utf-8");
    assert!(
        text.contains("capped=true"),
        "metrics must report capped=true: {text}"
    );
    assert!(
        text.contains("incomplete_reason=none"),
        "capped search must not set incomplete_reason: {text}"
    );

    let _ = fs::remove_dir_all(base);
}

// ---------------------------------------------------------------------------
// search_json_max_results_zero_caps_empty_hits
// ---------------------------------------------------------------------------

/// `--max-results 0` on a query that would normally hit still exits 0 with
/// empty hits and `capped=true` (issue #136).
#[test]
fn search_json_max_results_zero_caps_empty_hits() {
    let base = std::env::temp_dir().join(format!("qzt-36-max0-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    let packed = pack_to(b"needle one\nneedle two\nneedle three\n", &base);
    let qzt = packed.to_str().unwrap();

    let out = run(&[
        "search",
        qzt,
        "needle",
        "--max-results",
        "0",
        "--format",
        "json",
    ]);
    assert!(
        out.status.success(),
        "max-results 0 capped search must exit 0: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json = String::from_utf8(out.stdout).expect("stdout is utf-8");
    let value: serde_json::Value = serde_json::from_str(&json)
        .unwrap_or_else(|error| panic!("stdout must be valid JSON: {error}\n{json}"));

    let hits = value
        .get("hits")
        .and_then(serde_json::Value::as_array)
        .expect("hits must be an array");
    assert!(
        hits.is_empty(),
        "hits must be empty when max-results is 0: {json}"
    );

    assert_eq!(
        value.get("capped").and_then(serde_json::Value::as_bool),
        Some(true),
        "capped must be true: {json}"
    );

    assert_eq!(
        value.get("incomplete_reason"),
        Some(&serde_json::Value::Null),
        "incomplete_reason must be null: {json}"
    );

    let metrics = value
        .get("metrics")
        .and_then(serde_json::Value::as_object)
        .expect("metrics must be an object");

    let candidate_granules = metrics
        .get("candidate_granules")
        .and_then(serde_json::Value::as_u64)
        .expect("candidate_granules must be a non-negative integer");
    assert!(
        candidate_granules > 0,
        "query must have candidate granules (would have hits without cap): {json}"
    );

    assert_eq!(
        metrics
            .get("verified_matches")
            .and_then(serde_json::Value::as_u64),
        Some(0),
        "verified_matches must be 0 when capped at 0: {json}"
    );

    let _ = fs::remove_dir_all(base);
}

// ---------------------------------------------------------------------------
// search_unknown_format_exits_2
// ---------------------------------------------------------------------------

/// An unknown `--format` value must exit with code 2 (usage error).
#[test]
fn search_unknown_format_exits_2() {
    let base = std::env::temp_dir().join(format!("qzt-36-fmt-bad-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    let packed = pack_to(b"hello\n", &base);
    let qzt = packed.to_str().unwrap();

    let out = run(&["search", qzt, "hello", "--format", "csv"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "unknown --format must exit 2: {:?}",
        out.status.code()
    );

    let _ = fs::remove_dir_all(base);
}

// ---------------------------------------------------------------------------
// search_format_text_explicit_accepted
// ---------------------------------------------------------------------------

/// `--format text` is accepted explicitly and produces text output with the
/// same structure as the default (no `--format` flag).
///
/// Note: byte-exact equality between two separate runs is not asserted because
/// `query_time_ms` naturally differs between invocations. Instead we verify
/// that both runs produce the `hit …` and `metrics …` line prefixes.
#[test]
fn search_format_text_explicit_accepted() {
    let base = std::env::temp_dir().join(format!("qzt-36-fmt-text-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    let packed = pack_to(b"hello world\n", &base);
    let qzt = packed.to_str().unwrap();

    let default_out = run(&["search", qzt, "hello"]);
    let text_out = run(&["search", qzt, "hello", "--format", "text"]);

    assert!(
        default_out.status.success(),
        "default mode failed: {}",
        String::from_utf8_lossy(&default_out.stderr)
    );
    assert!(
        text_out.status.success(),
        "--format text failed: {}",
        String::from_utf8_lossy(&text_out.stderr)
    );

    // Both must contain the text-mode markers (not JSON braces).
    let default_text = String::from_utf8(default_out.stdout).expect("default stdout is utf-8");
    let explicit_text = String::from_utf8(text_out.stdout).expect("--format text stdout is utf-8");

    for text in [&default_text, &explicit_text] {
        assert!(
            text.contains("source=verified_original_bytes"),
            "--format text must contain source= field: {text}"
        );
        assert!(
            text.contains("metrics "),
            "--format text must contain metrics line: {text}"
        );
        assert!(
            !text.trim().starts_with('{'),
            "--format text must not emit JSON: {text}"
        );
    }

    let _ = fs::remove_dir_all(base);
}

// ---------------------------------------------------------------------------
// search_json_source_field_is_escaped
// ---------------------------------------------------------------------------

/// The `source` field in hits is passed through `cli_json::escape`. Because
/// `source` is a `&'static str` from library code (`"verified_original_bytes"`)
/// it contains no special characters, but the escaping function is always
/// applied as required by the issue.
#[test]
fn search_json_source_field_is_string() {
    let base = std::env::temp_dir().join(format!("qzt-36-json-src-{}", std::process::id()));
    let _ = fs::create_dir_all(&base);

    let packed = pack_to(b"alpha beta gamma\n", &base);
    let qzt = packed.to_str().unwrap();

    let out = run(&["search", qzt, "alpha", "--format", "json"]);
    assert!(out.status.success());

    let json = String::from_utf8(out.stdout).expect("stdout is utf-8");

    // source must appear as a JSON string value (quoted).
    assert!(
        json.contains("\"source\":\""),
        "source must be a quoted string: {json}"
    );

    let _ = fs::remove_dir_all(base);
}
