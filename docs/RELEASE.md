# QZT Release Checklist

日本語版: [RELEASE.ja.md](RELEASE.ja.md)

This runbook prepares and publishes QZT v0.1.0 without blurring the boundary
between reversible validation and the irreversible crates.io upload. QZT v0.1
must be described as a technical preview, not as production-ready software.

## Gate ownership

Only the release owner may approve the dedicated pull request that removes
`publish = false` and run the real `cargo publish`. A publishable stable
manifest and a successful dry-run are not approval to upload. Never paste a
crates.io token into an issue, pull request, terminal transcript, or CI log.

Issue #42 proved packaging readiness while deliberately preserving
`publish = false`. The dedicated stable release pull request intentionally
keeps publication eligibility after removing that guard; actual upload remains
a separate release-owner-only command from its exact merge commit.

## Merge and publication prerequisites

- [ ] Refactoring issue #22 (public pack API consolidation) is merged.
- [ ] Refactoring issue #30 (public rustdoc and lint cleanup) is merged.
- [ ] The release owner has explicitly approved crates.io publication.
- [ ] The `qzt` name is still unclaimed immediately before opening the release
      pull request. Check both `https://crates.io/crates/qzt` and
      `https://index.crates.io/3/q/qzt`; escalate any conflict instead of
      choosing a new name.
- [ ] `main` is clean, up to date, and all required GitHub checks are green.
- [ ] The version remains `0.1.0` and the product is still presented as a
      technical preview.

Reversible candidate preparation and dry-runs may run before owner approval.
Do not merge the stable release pull request or publish while any prerequisite
is unchecked.

## Reversible preparation

Run from a clean checkout of the intended release commit:

```sh
git status --porcelain
make check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo package --list
cargo package --locked
```

The first command must print nothing. The default-feature rustdoc command is
the authoritative public API gate. The all-features command is an additional
compile check for hidden conformance internals; it must never replace the
default-feature gate.

Review the complete `cargo package --list` output. It must include at least
`Cargo.toml`, `Cargo.lock`, `README.md`, both license files, `CHANGELOG.md`,
`src/`, and the portable tests/vectors needed by downstream implementers. It
must not include `.github/`, `fuzz/`, `tasks/`, or either full Core Spec file.
Record the file count and compressed `.crate` size in the release pull request.

On the dedicated stable release commit, confirm the manifest is intentionally
publishable and run the dry-run from a clean worktree:

```sh
cargo metadata --no-deps --format-version 1
cargo publish --dry-run --locked
```

The effective `qzt` package metadata must report version `0.1.0` with no
publication allow-list or deny-list. Publication eligibility is an intentional
release input and must not be restored to `publish = false` after the dry-run.
Prove that the dry-run left the exact reviewed commit unchanged:

```sh
git status --porcelain
git diff --exit-code HEAD --
```

Both commands must print nothing.

Attach the dry-run result, package file count, package size, and reviewed
exclusions to issue #42. Do not include credentials or unrelated environment
data in that evidence.

## GitHub binary prerelease rehearsal

Issue #43 is a reversible GitHub-only rehearsal and does not authorize a
crates.io upload. Its manifest version is `0.1.0-pre.2`. A separate,
owner-approved release pull request restores the stable `0.1.0` version only
after this rehearsal and every other release prerequisite succeed.

The generated `.github/workflows/release.yml` has a tag-push trigger and no
branch or pull-request trigger. `make dist-check` regenerates it and reapplies
the repository's least-privilege and digest-pin hardening. The `release`
environment accepts only `v0.1.0-pre.*` tags and requires release-owner review
before the write-enabled host job can create a Release. After the issue #43
pull request is reviewed and merged, tag that exact merge commit and push only
the rehearsal tag:

```sh
git switch main
git pull --ff-only origin main
git status --short
git tag --annotate v0.1.0-pre.2 -m "qzt v0.1.0-pre.2"
git push origin v0.1.0-pre.2
```

- [ ] The Release is marked as a prerelease.
- [ ] `make dist-check` confirms the generated workflow plus hardening is current.
- [ ] The release owner approves the protected `release` environment deployment.
- [ ] All four target archives and their `.sha256` sidecars are present.
- [ ] The extracted binary reports `qzt 0.1.0-pre.2` from `qzt --version`.
- [ ] The Linux artifact links only to `libc.so.6` and Rust's GNU unwind
      runtime `libgcc_s.so.1`; zstd is statically linked into the binary.

Delete neither the rehearsal tag nor its Release after publishing: they are
immutable evidence for the distribution path. A failed rehearsal is fixed by
a new commit and a new prerelease version, never by moving the published tag.

## Owner-approved release pull request

After every prerequisite is satisfied, prepare a dedicated release pull
request that:

- [ ] changes `## 0.1.0 - Unreleased` to `## 0.1.0 - YYYY-MM-DD`;
- [ ] removes `publish = false`;
- [ ] contains no unrelated code or API changes;
- [ ] repeats `make check`, warning-free rustdoc, `cargo package --list`, and
      `cargo publish --dry-run --locked` from a clean commit;
- [ ] records the exact commit SHA whose package was reviewed;
- [ ] receives explicit approval from the release owner before merge.
- [ ] adds an exact `v0.1.0` tag policy to the protected `release` environment
      while retaining the required release-owner reviewer.

## Irreversible publication — release owner only

From the clean, exact merge commit of the approved release pull request:

```sh
git switch main
git pull --ff-only origin main
git status --short
cargo publish --locked
```

- [ ] Confirm `cargo publish` succeeds and `qzt 0.1.0` appears on crates.io.
- [ ] Confirm the docs.rs build succeeds and renders the public API.
- [ ] Create a signed or annotated tag on the exact published commit, only
      after the successful upload:

```sh
git tag -a v0.1.0 -m "qzt v0.1.0"
git push origin v0.1.0
```

- [ ] Create the GitHub Release and attach the checksum-bearing binaries from
      issue #43.
- [ ] Add crates.io and docs.rs installation links/badges during issue #44.
- [ ] Run an installation smoke test in a new temporary directory:

```sh
cargo install qzt --version 0.1.0 --locked
qzt --version
```

If a post-publish defect is discovered, do not overwrite the version (crates.io
does not permit that). Follow the crates.io yank policy and prepare a new patch
release; preserve the incident evidence.
