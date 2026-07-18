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
