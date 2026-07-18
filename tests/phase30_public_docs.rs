fn workspace_file(path: &str) -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    std::fs::read_to_string(format!("{root}/{path}"))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn default_public_surface_denies_missing_documentation() {
    let lib = workspace_file("src/lib.rs");

    assert!(
        lib.contains("#![cfg_attr(not(feature = \"internal-testing\"), deny(missing_docs))]"),
        "the curated default public API must reject undocumented additions"
    );
    assert!(
        !lib.contains("warn(missing_docs)"),
        "missing documentation must not remain a warning-only gate"
    );
}

#[test]
fn internal_module_declarations_do_not_hide_dead_code() {
    let lib = workspace_file("src/lib.rs");
    assert!(
        !lib.contains("allow(dead_code)"),
        "module declarations must not hide dead code in any attribute form; narrow item-level exceptions belong beside the justified item"
    );
}

#[test]
fn temporary_public_documentation_lint_allows_are_removed() {
    let manifest = workspace_file("Cargo.toml");

    for lint in ["missing_errors_doc", "missing_panics_doc"] {
        assert!(
            !manifest
                .lines()
                .any(|line| line.trim_start().starts_with(lint)),
            "{lint} must be enforced instead of allowed"
        );
    }
}

#[test]
fn local_quality_gate_treats_rustdoc_warnings_as_errors() {
    let makefile = workspace_file("Makefile");

    assert!(
        makefile.contains("RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps --all-features"),
        "the documented local rustdoc gate must remain warning-free"
    );
    assert!(
        !makefile.contains("missing_docs warnings are a known deferred backlog"),
        "the completed backlog must not remain documented as deferred"
    );
}
