use std::fs;

fn repository_file(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn ci_runs_the_pinned_local_coverage_gate() {
    let workflow = repository_file(".github/workflows/ci.yml");

    assert!(workflow.contains("  coverage:\n"));
    assert!(workflow.contains("components: llvm-tools-preview"));
    assert!(
        workflow.contains("uses: taiki-e/install-action@07b4745e0c39a41822af610387492e3e53aa222b")
    );
    assert!(workflow.contains("tool: cargo-llvm-cov@0.8.6"));
    assert!(workflow.contains("fallback: none"));
    assert!(workflow.contains("run: make coverage"));
}

#[test]
fn make_target_enforces_the_measured_line_coverage_floor() {
    let makefile = repository_file("Makefile");

    assert!(makefile.contains("coverage:\n"));
    assert!(makefile.contains("cargo llvm-cov --all-features --workspace --fail-under-lines 90"));
}

#[test]
fn contributor_guides_document_the_reproducible_coverage_gate() {
    assert!(repository_file("CONTRIBUTING.md").contains("make coverage"));
    assert!(repository_file("CONTRIBUTING.ja.md").contains("make coverage"));
}
