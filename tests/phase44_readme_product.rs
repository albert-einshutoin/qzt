const ENGLISH: &str = include_str!("../README.md");
const JAPANESE: &str = include_str!("../README.ja.md");

#[test]
fn readmes_lead_with_product_value_and_live_distribution() {
    for readme in [ENGLISH, JAPANESE] {
        for requirement in [
            "actions/workflows/ci.yml/badge.svg?branch=main",
            "v0.1.0-pre.2",
            "qzt-installer.sh",
            "checksum",
            "technical preview",
        ] {
            assert!(
                readme.contains(requirement),
                "missing README value: {requirement}"
            );
        }
        assert!(!readme.contains("become available after"));
        assert!(!readme.contains("完了した後に利用できます"));
    }

    assert!(ENGLISH.starts_with("# QZT — Cold Evidence Container for Text"));
    assert!(JAPANESE.starts_with("# QZT — テキストのための Cold Evidence Container"));
    assert!(ENGLISH.contains("Store large text once, prove it later"));
    assert!(JAPANESE.contains("大きなテキストを一度保存し、あとから証明する"));
}

#[test]
fn readmes_share_the_same_product_journey() {
    let english = [
        "## Why QZT",
        "## Install",
        "## 60-second Tour",
        "## Use Cases",
        "## Status & Limitations",
        "## CLI Reference",
        "## Documentation",
        "## Development",
    ];
    let japanese = [
        "## QZTを選ぶ理由",
        "## Install / インストール",
        "## 60秒ツアー",
        "## ユースケース",
        "## ステータスと制限",
        "## CLIリファレンス",
        "## ドキュメント",
        "## 開発",
    ];

    assert_in_order(ENGLISH, &english);
    assert_in_order(JAPANESE, &japanese);
}

#[test]
fn tour_closes_the_verified_evidence_loop_with_real_commands() {
    for readme in [ENGLISH, JAPANESE] {
        for command in [
            "printf 'alpha\\nbeta\\nerror gamma\\n' > app.log",
            "qzt pack app.log -o app.qzt",
            "qzt info app.qzt --format json",
            "qzt range app.qzt --lines 2:2",
            "qzt sidecar-rebuild app.qzt -o app.qzt.qzi",
            "qzt search app.qzt \"error\" --sidecar app.qzt.qzi",
            "qzt verify app.qzt --deep",
            "qzt attest app.qzt > app.attest.json",
        ] {
            assert!(
                readme.contains(command),
                "missing executable tour command: {command}"
            );
        }
    }

    for readme in [ENGLISH, JAPANESE] {
        assert!(readme.contains("docs/benchmarks/2026-07-v0.1.md"));
        assert!(readme.contains("Tantivy"));
        assert!(readme.contains("Lucene"));
        assert!(readme.contains("seekable-zstd"));
    }
    for readme in [ENGLISH, JAPANESE] {
        assert!(readme.contains("cargo build --release"));
        assert!(readme.contains("docs/guides/attestation.md"));
    }
}

fn assert_in_order(document: &str, headings: &[&str]) {
    let mut previous = 0;
    for heading in headings {
        let position = document
            .find(heading)
            .unwrap_or_else(|| panic!("missing heading: {heading}"));
        assert!(position >= previous, "heading out of order: {heading}");
        previous = position;
    }
}
