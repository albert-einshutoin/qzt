use std::fs;

fn repository_file(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn default_local_gate_includes_rustdoc_warnings() {
    let makefile = repository_file("Makefile");
    assert!(makefile.contains("check: fmt clippy check-default test doc"));
    assert!(makefile.contains("RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps --all-features"));
}

#[test]
fn quick_profile_target_reuses_the_exact_profile_path_with_small_counts() {
    let makefile = repository_file("Makefile");
    let recipe = makefile
        .split("\nbench-profile-quick:")
        .nth(1)
        .and_then(|tail| tail.split("\nbench-profile-matrix:").next())
        .expect("bench-profile-quick recipe must precede the matrix target");

    assert!(recipe.contains("QZT_RELEASE_BENCH_QUERY_REPETITIONS=5"));
    assert!(recipe.contains("QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS=2"));
    assert!(recipe.contains("$(MAKE) bench-profile"));
    assert!(repository_file("README.md").contains("make bench-profile-quick"));
    assert!(repository_file("README.ja.md").contains("make bench-profile-quick"));
}

#[test]
fn semgrep_ignore_excludes_only_generated_non_source_state() {
    let ignore = repository_file(".semgrepignore");

    for generated in [
        "/target/",
        "/fuzz/target/",
        "/fuzz/corpus/",
        "/fuzz/artifacts/",
        "/coverage/",
        "/coverage-*/",
    ] {
        assert!(ignore.contains(generated), "missing {generated}");
    }

    for source in ["src/", "tests/", "docs/", ".github/"] {
        assert!(!ignore.contains(source), "must keep {source} scan-visible");
    }
}
