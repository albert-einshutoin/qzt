//! Run the public-history example with the repository's built CLI.
#[test]
fn public_history_evidence_handoff_and_failure_boundaries() {
    let python = if cfg!(windows) { "python" } else { "python3" };
    let output = std::process::Command::new(python)
        .args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "examples/public-history-evidence",
            "-p",
            "test_case.py",
        ])
        .env("QZT_PUBLIC_HISTORY_BINARY", env!("CARGO_BIN_EXE_qzt"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Python 3 is required for the public-history handoff E2E");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
