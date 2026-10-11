//! Execute the standalone recipient with only its handoff, using Python's existing unittest style.
#[test]
fn decision_evidence_handoff_and_failure_boundaries() {
    let python = if cfg!(windows) { "python" } else { "python3" };
    let output = std::process::Command::new(python)
        .args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "examples/decision-evidence",
            "-p",
            "test_case.py",
        ])
        .env("QZT_DECISION_BINARY", env!("CARGO_BIN_EXE_qzt"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Python 3 is required for the documented handoff E2E");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
