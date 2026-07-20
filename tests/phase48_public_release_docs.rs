use std::fs;
use std::path::Path;

const README_EN: &str = include_str!("../README.md");
const README_JA: &str = include_str!("../README.ja.md");

#[test]
fn readmes_define_non_goals_and_link_release_contracts() {
    for requirement in [
        "docs/QZT_v0.1_Format_Stability.md",
        "docs/QZT_v0.1_Memory_Guarantees.md",
        "docs/QZT_v0.1_Competitive_Benchmarks.md",
        "docs/QZT_v0.1_Validation_Corpus.md",
        "docs/API_STABILITY.md",
    ] {
        assert!(
            README_EN.contains(requirement),
            "English README misses {requirement}"
        );
        assert!(
            README_JA.contains(requirement),
            "Japanese README misses {requirement}"
        );
    }

    for requirement in [
        "## What QZT is not",
        "not a replacement for zstd",
        "does not display text without decompression",
        "not Memory Pager",
        "not a vector database or FM-index",
        "v0.1 technical preview",
    ] {
        assert!(
            README_EN.contains(requirement),
            "English README misses {requirement:?}"
        );
    }

    for requirement in [
        "## QZTがしないこと",
        "zstdの代替ではありません",
        "解凍せずにテキストを表示するものではありません",
        "Memory Pagerではありません",
        "vector databaseやFM-indexではありません",
        "v0.1 technical preview",
    ] {
        assert!(
            README_JA.contains(requirement),
            "Japanese README misses {requirement:?}"
        );
    }

    assert_heading_outside_code_fence(README_EN, "## What QZT is not");
    assert_heading_outside_code_fence(README_JA, "## QZTがしないこと");
}

fn assert_heading_outside_code_fence(document: &str, heading: &str) {
    let heading_offset = document.find(heading).expect("heading should exist");
    let fence_count = document[..heading_offset].matches("```").count();
    assert_eq!(
        fence_count % 2,
        0,
        "heading {heading:?} must not be inside a fenced code block"
    );
}

#[test]
fn japanese_contributor_guide_preserves_the_repository_contract() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("CONTRIBUTING.ja.md");
    assert!(path.is_file(), "missing {}", path.display());
    let guide = fs::read_to_string(path).expect("Japanese contributor guide should be readable");

    for requirement in [
        "GitHub Flow",
        "開発契約",
        "make check",
        "conformance test",
        "Security Policy",
        "Semgrep",
        "OSV Scanner",
        "Gitleaks",
        "release",
        "technical preview",
        "[English](CONTRIBUTING.md)",
    ] {
        assert!(
            guide.contains(requirement),
            "Japanese contributor guide misses {requirement:?}"
        );
    }

    assert!(README_JA.contains("[コントリビューションガイド](CONTRIBUTING.ja.md)"));
    assert!(README_EN.contains("[Contributing Guide](CONTRIBUTING.md)"));
}
