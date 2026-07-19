use std::fs;
use std::path::Path;

const README: &str = include_str!("../README.md");
const CONTRIBUTING: &str = include_str!("../CONTRIBUTING.md");
const SECURITY: &str = include_str!("../SECURITY.md");

#[test]
fn public_docs_link_to_private_vulnerability_reporting() {
    for document in [README, CONTRIBUTING] {
        assert!(
            document.contains("[Security Policy](SECURITY.md)"),
            "public contributor entry points must link to SECURITY.md"
        );
    }

    let security = SECURITY.split_whitespace().collect::<Vec<_>>().join(" ");
    for requirement in [
        "GitHub security advisory",
        "privately",
        "minimal reproducer",
        "expected impact",
        "untrusted `.qzt`",
        "Public issues",
    ] {
        assert!(
            security.contains(requirement),
            "SECURITY.md is missing {requirement:?}"
        );
    }
}

#[test]
fn pull_request_template_enforces_the_development_contract() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/pull_request_template.md");
    assert!(path.is_file(), "missing {}", path.display());
    let template = fs::read_to_string(path).expect("PR template should be readable");

    for requirement in [
        "Why",
        "Before",
        "After",
        "self-review",
        "architecture review",
        "make check",
        "security",
        "secrets",
        "generated files",
        "status",
    ] {
        assert!(
            template.contains(requirement),
            "PR template is missing {requirement:?}"
        );
    }
}
