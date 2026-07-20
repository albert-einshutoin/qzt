use std::fs;

fn repository_file(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn normal_ci_executes_the_public_evidence_example_once() {
    let workflow = repository_file(".github/workflows/ci.yml");

    assert!(workflow.contains("if: matrix.toolchain == 'stable'"));
    assert!(workflow.contains("run: cargo run --locked --example evidence_ref"));
}

#[test]
fn fuzz_smoke_is_bounded_scheduled_manual_and_reproducible() {
    let workflow = repository_file(".github/workflows/fuzz-smoke.yml");

    assert!(workflow.contains("  workflow_dispatch:\n"));
    assert!(workflow.contains("  schedule:\n"));
    assert!(!workflow.contains("  pull_request:\n"));
    assert!(!workflow.contains("  push:\n"));
    assert!(workflow.contains("permissions:\n  contents: read"));
    assert!(workflow.contains("timeout-minutes: 10"));
    assert!(workflow.contains("toolchain: nightly"));
    assert!(workflow.contains("CARGO_FUZZ_VERSION: \"0.13.2\""));
    assert!(workflow.contains("cargo +nightly metadata --manifest-path fuzz/Cargo.toml --locked"));
    assert!(workflow.contains("cargo +nightly fuzz build open_verify"));
    assert!(workflow.contains(
        "cargo +nightly fuzz run open_verify -- -max_total_time=60 -timeout=10 -max_len=4096"
    ));
    assert!(
        workflow.contains("uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02")
    );
}

#[test]
fn generated_fuzz_state_is_ignored_and_contributor_commands_are_documented() {
    let ignore = repository_file(".gitignore");
    assert!(ignore.contains("/fuzz/corpus/"));
    assert!(repository_file("fuzz/Cargo.lock").contains("name = \"qzt-fuzz\""));
    assert!(
        repository_file("fuzz/fuzz_targets/open_verify.rs")
            .contains("const MAX_ROUND_TRIP_BYTES: usize = 4 * 1024;")
    );

    for guide in ["CONTRIBUTING.md", "CONTRIBUTING.ja.md"] {
        let contents = repository_file(guide);
        assert!(contents.contains("cargo run --locked --example evidence_ref"));
        assert!(contents.contains(
            "cargo +nightly fuzz run open_verify -- -max_total_time=60 -timeout=10 -max_len=4096"
        ));
    }
}

#[test]
fn fuzz_workspace_is_licensed_versioned_and_dependency_audited() {
    let manifest = repository_file("fuzz/Cargo.toml");
    assert!(manifest.contains("license = \"MIT OR Apache-2.0\""));
    assert!(manifest.contains("qzt = { path = \"..\", version = \"=0.1.0\" }"));

    let policy = repository_file("deny.toml");
    assert!(policy.contains("\"NCSA\""));

    let ci = repository_file(".github/workflows/ci.yml");
    assert!(ci.contains("manifest-path: fuzz/Cargo.toml"));
}
