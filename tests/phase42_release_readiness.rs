const MANIFEST: &str = include_str!("../Cargo.toml");
const RELEASE_GUIDE: &str = include_str!("../docs/RELEASE.md");
const CHANGELOG: &str = include_str!("../CHANGELOG.md");

#[test]
fn manifest_is_discoverable_but_cannot_be_published_from_this_change() {
    for metadata in [
        "description = \"Cold evidence container for seekable, verifiable UTF-8 text archives\"",
        "documentation = \"https://docs.rs/qzt\"",
        "homepage = \"https://github.com/albert-einshutoin/qzt\"",
        "repository = \"https://github.com/albert-einshutoin/qzt\"",
        "license = \"MIT OR Apache-2.0\"",
        "readme = \"README.md\"",
    ] {
        assert!(MANIFEST.contains(metadata), "missing package metadata: {metadata}");
    }

    assert!(
        MANIFEST.contains("publish = false"),
        "release-readiness work must not open the irreversible publish gate"
    );
}

#[test]
fn package_excludes_repository_only_material() {
    for excluded in [
        "\".github/\"",
        "\"fuzz/\"",
        "\"tasks/\"",
        "\"docs/QZT_v0.1_Core_Spec.md\"",
        "\"docs/QZT_v0.1_Core_Spec.ja.md\"",
    ] {
        assert!(MANIFEST.contains(excluded), "missing package exclusion: {excluded}");
    }
}

#[test]
fn release_guide_preserves_owner_gate_and_dependency_checks() {
    for requirement in [
        "#22",
        "#30",
        "cargo publish --dry-run",
        "cargo package --list",
        "cargo doc --no-deps --all-features",
        "publish = false",
        "release owner",
        "cargo publish",
        "docs.rs",
        "v0.1.0",
    ] {
        assert!(
            RELEASE_GUIDE.contains(requirement),
            "release guide is missing: {requirement}"
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
fn changelog_points_release_owners_to_the_new_gate() {
    assert!(CHANGELOG.contains("Ready for owner-gated crates.io publication"));
    assert!(CHANGELOG.contains("[release checklist](docs/RELEASE.md)"));
    assert!(!CHANGELOG.contains(
        "crates.io publication and publish dry-run until Phase20 stabilizes"
    ));
}
