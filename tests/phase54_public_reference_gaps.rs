use std::fs;
use std::path::Path;

fn repository_file(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn japanese_competitive_benchmark_preserves_the_public_scope() {
    let path = "docs/QZT_v0.1_Competitive_Benchmarks.ja.md";
    assert!(Path::new(path).is_file(), "missing {path}");

    let japanese = repository_file(path);
    for required in [
        "[English](QZT_v0.1_Competitive_Benchmarks.md)",
        "cargo test --features internal-testing --test phase18_competitive_benchmark -- --nocapture",
        "cargo test --release --all-features --test phase18_competitive_benchmark -- --nocapture",
        "bench-compete",
        "SQLite FTS5",
        "ripgrep",
        "SLA",
    ] {
        assert!(japanese.contains(required), "missing {required}");
    }
}

#[test]
fn docs_index_exposes_normative_and_companion_contracts() {
    let index = repository_file("docs/README.md");

    for required in [
        "v0.1 technical preview",
        "English Core specification is normative",
        "QZT_v0.1_Core_Spec.md",
        "QZT_v0.1_Core_Spec.ja.md",
        "QZI_v0.1_Sidecar_Spec.md",
        "API_STABILITY.md",
        "QZT_v0.1_Competitive_Benchmarks.ja.md",
        "Security_CI_Playbook.md",
    ] {
        assert!(index.contains(required), "missing {required}");
    }

    assert!(repository_file("README.md").contains("[Documentation index](docs/README.md)"));
    assert!(repository_file("README.ja.md").contains("[ドキュメント索引](docs/README.md)"));
}

#[test]
fn readmes_disclose_search_and_empty_line_boundaries() {
    let english = repository_file("README.md");
    let japanese = repository_file("README.ja.md");

    for readme in [&english, &japanese] {
        for required in [
            "ASCII",
            "`_`",
            "`-`",
            "n-gram",
            "query_has_no_indexable_tokens",
            "query_shorter_than_ngram_n",
            "exit code **1**",
        ] {
            assert!(readme.contains(required), "missing {required}");
        }
    }

    assert!(english.contains("empty container"));
    assert!(japanese.contains("空のcontainer"));
}
