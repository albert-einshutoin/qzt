use std::fs;

fn workflow(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn assert_scoped_push_and_main_pull_request(path: &str, push_branches: &str) {
    let yaml = workflow(path);

    assert!(
        yaml.contains(&format!("  push:\n    branches: {push_branches}\n")),
        "{path} must keep push checks on its explicit post-merge/release branches"
    );
    assert!(
        yaml.contains("  pull_request:\n    branches: [main]\n"),
        "{path} must run pull-request checks for changes targeting main"
    );
}

#[test]
fn ci_does_not_run_both_push_and_pull_request_for_feature_branches() {
    assert_scoped_push_and_main_pull_request(".github/workflows/ci.yml", "[main, \"release/**\"]");
}

#[test]
fn security_keeps_manual_and_scheduled_scans_without_duplicate_pr_runs() {
    let path = ".github/workflows/security.yml";
    assert_scoped_push_and_main_pull_request(path, "[main]");

    let yaml = workflow(path);
    assert!(yaml.contains("  workflow_dispatch:\n"));
    assert!(yaml.contains("  schedule:\n"));
}
