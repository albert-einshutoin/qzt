use std::fs;

fn repository_file(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn ci_enforces_dependency_license_ban_and_source_policy() {
    let workflow = repository_file(".github/workflows/ci.yml");

    assert!(workflow.contains("  dependency-policy:\n"));
    assert!(workflow.contains(
        "uses: EmbarkStudios/cargo-deny-action@3c6349835b2b7b196a839186cb8b78e02f7b5f25"
    ));
    assert!(workflow.contains("rust-version: \"1.87.0\""));
    assert!(workflow.contains("command: check bans licenses sources"));
}

#[test]
fn deny_config_is_fail_closed_for_licenses_and_dependency_sources() {
    let config = repository_file("deny.toml");

    assert!(config.contains("[licenses]"));
    assert!(config.contains("[bans]"));
    assert!(config.contains("[sources]"));
    assert!(config.contains("unknown-registry = \"deny\""));
    assert!(config.contains("unknown-git = \"deny\""));
}

#[test]
fn contributor_guide_documents_the_local_dependency_gate() {
    let guide = repository_file("CONTRIBUTING.md");
    let japanese_guide = repository_file("CONTRIBUTING.ja.md");

    assert!(guide.contains("cargo deny check bans licenses sources"));
    assert!(japanese_guide.contains("cargo deny check bans licenses sources"));
}
