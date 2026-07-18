use std::process::Command;

const MANIFEST: &str = include_str!("../Cargo.toml");
const RELEASE_GUIDE: &str = include_str!("../docs/RELEASE.md");
const JAPANESE_RELEASE_GUIDE: &str = include_str!("../docs/RELEASE.ja.md");
const CHANGELOG: &str = include_str!("../CHANGELOG.md");
const README: &str = include_str!("../README.md");
const JAPANESE_README: &str = include_str!("../README.ja.md");
const VECTOR_README: &str = include_str!("vectors/README.md");

#[test]
fn owner_approved_release_manifest_is_stable_and_publishable() {
    for metadata in [
        "description = \"Cold evidence container for seekable, verifiable UTF-8 text archives\"",
        "documentation = \"https://docs.rs/qzt\"",
        "homepage = \"https://github.com/albert-einshutoin/qzt\"",
        "repository = \"https://github.com/albert-einshutoin/qzt\"",
        "license = \"MIT OR Apache-2.0\"",
        "readme = \"README.md\"",
    ] {
        assert!(
            MANIFEST.contains(metadata),
            "missing package metadata: {metadata}"
        );
    }

    let metadata = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .expect("cargo metadata must run");
    assert!(metadata.status.success(), "cargo metadata must succeed");
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata.stdout).expect("cargo metadata must be JSON");
    let package = metadata["packages"]
        .as_array()
        .and_then(|packages| packages.iter().find(|package| package["name"] == "qzt"))
        .expect("cargo metadata must contain the qzt package");
    assert_eq!(
        package["version"],
        serde_json::json!("0.1.0"),
        "the owner-approved release manifest must use the stable version"
    );
    assert_eq!(
        package["publish"],
        serde_json::Value::Null,
        "the owner-approved release manifest must not block crates.io publication"
    );
}

#[test]
fn docs_rs_builds_the_public_default_feature_surface() {
    let docs_rs = MANIFEST
        .split_once("[package.metadata.docs.rs]")
        .and_then(|(_, rest)| rest.split("\n[").next())
        .expect("docs.rs metadata section must exist");
    assert!(
        !docs_rs.contains("all-features"),
        "docs.rs must not enable the internal-testing feature"
    );
}

#[test]
fn package_excludes_repository_only_material() {
    let package = Command::new(env!("CARGO"))
        .args(["package", "--allow-dirty", "--list"])
        .output()
        .expect("cargo package --list must run");
    assert!(
        package.status.success(),
        "cargo package --list must succeed"
    );
    let packaged_files = String::from_utf8(package.stdout).expect("package list must be UTF-8");

    // Test Cargo's effective package, not just matching strings in Cargo.toml:
    // opening a metadata table above `exclude` silently changes its ownership.
    for excluded in [
        ".github/",
        "fuzz/",
        "scripts/",
        "tasks/",
        "docs/QZT_v0.1_Core_Spec.md",
        "docs/QZT_v0.1_Core_Spec.ja.md",
    ] {
        assert!(
            !packaged_files
                .lines()
                .any(|path| path.starts_with(excluded)),
            "repository-only material leaked into the package: {excluded}"
        );
    }
}

#[test]
fn release_guide_preserves_owner_gate_and_dependency_checks() {
    for guide in [RELEASE_GUIDE, JAPANESE_RELEASE_GUIDE] {
        for requirement in [
            "#22",
            "#30",
            "cargo publish --dry-run",
            "cargo package --list",
            "cargo doc --no-deps\n",
            "cargo doc --no-deps --all-features",
            "cargo metadata --no-deps --format-version 1",
            "release owner",
            "cargo publish",
            "docs.rs",
            "v0.1.0",
            "https://crates.io/crates/qzt",
            "https://index.crates.io/3/q/qzt",
        ] {
            assert!(
                guide.contains(requirement),
                "release guide is missing: {requirement}"
            );
        }
    }

    assert!(RELEASE_GUIDE.contains("choosing a new name"));
    assert!(JAPANESE_RELEASE_GUIDE.contains("別名を選ばず"));
    for guide in [RELEASE_GUIDE, JAPANESE_RELEASE_GUIDE] {
        assert!(
            guide.matches("git status --porcelain").count() >= 2,
            "release evidence must prove the whole worktree is clean before and after dry-run"
        );
        assert!(
            !guide.contains("git restore Cargo.toml"),
            "the stable release runbook must preserve intentional publication eligibility"
        );
    }

    let publish = RELEASE_GUIDE
        .find("cargo publish` succeeds")
        .expect("guide must identify the successful publish event");
    let tag = RELEASE_GUIDE
        .find("git tag -a v0.1.0")
        .expect("guide must document the release tag");
    assert!(
        publish < tag,
        "the immutable tag must identify the exact commit that was published"
    );
}

#[test]
fn packaged_readmes_link_excluded_documents_to_the_repository() {
    for readme in [README, JAPANESE_README] {
        assert!(
            !readme.contains("](tasks/"),
            "packaged README must not use relative links into excluded tasks/"
        );
        assert!(
            !readme.contains("](docs/QZT_v0.1_Core_Spec"),
            "packaged README must not use relative links to an excluded Core Spec"
        );
    }

    assert!(
        !VECTOR_README.contains("](../../docs/QZT_v0.1_Core_Spec.md)"),
        "packaged vector guide must not link relatively to the excluded Core Spec"
    );
    assert!(
        VECTOR_README.contains(
            "https://github.com/albert-einshutoin/qzt/blob/main/docs/QZT_v0.1_Core_Spec.md"
        )
    );

    for (readme, excluded_path) in [
        (README, "docs/QZT_v0.1_Core_Spec.md"),
        (JAPANESE_README, "docs/QZT_v0.1_Core_Spec.ja.md"),
        (README, "tasks/README.md"),
        (README, "tasks/status.md"),
        (JAPANESE_README, "tasks/README.ja.md"),
        (JAPANESE_README, "tasks/status.ja.md"),
    ] {
        let absolute =
            format!("https://github.com/albert-einshutoin/qzt/blob/main/{excluded_path}");
        assert!(
            readme.contains(&absolute),
            "packaged README must not link relatively to excluded {excluded_path}"
        );
    }
}

#[test]
fn changelog_points_release_owners_to_the_new_gate() {
    assert!(CHANGELOG.contains("## 0.1.0 - 2026-07-19"));
    assert!(CHANGELOG.contains("Ready for owner-gated crates.io publication"));
    assert!(CHANGELOG.contains("[release checklist](docs/RELEASE.md)"));
    assert!(
        !CHANGELOG.contains("crates.io publication and publish dry-run until Phase20 stabilizes")
    );
}
