use std::fs;

fn workflow(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn assert_main_only_push_and_pull_request(path: &str) {
    let yaml = workflow(path);

    assert!(
        yaml.contains("  push:\n    branches: [main]\n"),
        "{path} must run push checks only after changes land on main"
    );
    assert!(
        yaml.contains("  pull_request:\n    branches: [main]\n"),
        "{path} must run pull-request checks for changes targeting main"
    );
}

#[test]
fn ci_does_not_run_both_push_and_pull_request_for_feature_branches() {
    assert_main_only_push_and_pull_request(".github/workflows/ci.yml");
}

#[test]
fn security_keeps_manual_and_scheduled_scans_without_duplicate_pr_runs() {
    let path = ".github/workflows/security.yml";
    assert_main_only_push_and_pull_request(path);

    let yaml = workflow(path);
    assert!(yaml.contains("  workflow_dispatch:\n"));
    assert!(yaml.contains("  schedule:\n"));
}
