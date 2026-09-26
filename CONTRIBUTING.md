# Contributing

[日本語](CONTRIBUTING.ja.md)

QZT is developed with GitHub Flow on short-lived feature branches. Keep changes
small, reviewable, and tied to the current [roadmap #31](https://github.com/albert-einshutoin/qzt/issues/31)
or the relevant child issue. The `tasks/` Phase and Post-Phase23 plans are
historical; [tasks/status.md](https://github.com/albert-einshutoin/qzt/blob/main/tasks/status.md)
summarizes current progress.

## Development Contract

Follow [AGENTS.md](AGENTS.md), #31, and the relevant issue for the verification
and review appropriate to the change. The old Phase workflow and its fixed
review-pass count do not queue new work or require a status edit for every PR.
Record detailed acceptance evidence on the child issue and PR; update the
status summary when the current state changes.

## Local Gate

Run the same gate CI runs:

```sh
make check
```

This default gate includes rustdoc with warnings denied; generated HTML stays
under the ignored Cargo target directory.

For documentation or release-hygiene changes, also run:

```sh
make doc
cargo package --allow-dirty
```

When changing `Cargo.toml`, `Cargo.lock`, or dependency policy, also run:

```sh
cargo deny check bans licenses sources
```

This gate reviews allowed licenses, banned or duplicated crates, and dependency
sources. OSV Scanner remains the vulnerability-advisory gate in CI.

To reproduce the CI line-coverage floor locally, install `cargo-llvm-cov` and
run:

```sh
make coverage
```

The initial measured line coverage was 92.37%; the gate starts at 90% to retain
roughly two percentage points of tolerance while blocking material regressions.

Before changing a public example, execute its user-visible path:

```sh
cargo run --locked --example evidence_ref
```

Changes to parsing, verification, or fuzz targets should replay the tracked
seeds and run the bounded nightly smoke locally when `cargo-fuzz` is available:

```sh
cargo test --manifest-path fuzz/Cargo.toml --test seed_replay --locked
mkdir -p fuzz/corpus/open_verify fuzz/corpus/qzi_search fuzz/corpus/dli_decode
for target in open_verify qzi_search dli_decode; do cp fuzz/seeds/$target/* fuzz/corpus/$target/; done
cargo +nightly fuzz run --sanitizer address open_verify fuzz/corpus/open_verify -- -max_total_time=60 -timeout=10 -max_len=4096 -rss_limit_mb=1024 -malloc_limit_mb=128 -seed=294
cargo +nightly fuzz run --sanitizer address qzi_search fuzz/corpus/qzi_search -- -max_total_time=60 -timeout=10 -max_len=256 -rss_limit_mb=1024 -malloc_limit_mb=128 -seed=294
cargo +nightly fuzz run --sanitizer address dli_decode fuzz/corpus/dli_decode -- -max_total_time=60 -timeout=10 -max_len=256 -rss_limit_mb=1024 -malloc_limit_mb=128 -seed=294
```

The same targets run weekly and on manual dispatch, not on every pull request.
See [fuzz/README.md](fuzz/README.md) for input modes, budgets, seed provenance,
reproduction and minimization. Generated corpus stays local; CI retains logs
and crash artifacts for seven days.

## Adding a conformance test

Conformance tests live under `tests/` as integration test binaries. Name new
files after the phase they belong to:

```text
tests/phase{N}_*.rs
```

Examples: `tests/phase9_hardening.rs`, `tests/phase22_vectors.rs`. Pick the
phase that matches the work in `tasks/` and the phase plan you are
implementing.

### Core conformance map

When a test provides evidence for a [Core conformance
item](docs/QZT_v0.1_Core_Spec.md#351-core-conformance-tests) (items 1–77),
update `CORE_CONFORMANCE_MAP` in `tests/phase9_hardening.rs`. Each entry is
`(item_number, description, evidence_test_name)` where `evidence_test_name` is
the Rust test path, for example
`phase5_writer::empty_file_pack_export_equality`.

`core_conformance_map_covers_all_items` asserts that the map lists items 1–77
in order with non-empty evidence. If you add or renumber a Core item, update
the map and run the hardening suite before the full gate.

### Verify your change

Run a focused test for the file you touched first, then the repository gate:

```sh
# replace phase9_hardening with your integration test binary name
cargo test --all-features --test phase9_hardening -- --nocapture

make check
```

When you change `CORE_CONFORMANCE_MAP`, include `phase9_hardening` in the
focused command even if the new evidence test lives in another phase file.

## Security Scans

Report suspected vulnerabilities privately through the
[Security Policy](SECURITY.md). Do not include exploit details or secrets in a
public issue or pull request.

CI runs Semgrep CE, OSV Scanner, and Gitleaks on pull requests, pushes,
scheduled scans, and manual dispatches.

For cross-repository selection criteria, policy levels, and a reusable GitHub
Actions template, see `docs/Security_CI_Playbook.md` and
`docs/Security_CI_Playbook.ja.md`.

Semgrep uses `semgrep scan --config p/rust --error` so findings fail the job.
The pinned scanner and the ten reviewed, call-local unsafe exceptions are
documented in the [#307 audit](docs/security/ffi-output-audit.md). A successful
scan does not mean that all `unsafe` has been removed or that vulnerabilities
cannot exist.

OSV Scanner checks `Cargo.lock` for known dependency vulnerabilities and fails
on reported vulnerabilities. This covers Rust dependency SCA; OWASP CVE Lite
CLI is intentionally not part of this workflow because it is focused on
JavaScript and TypeScript lockfiles (`package-lock.json`, `pnpm-lock.yaml`,
`yarn.lock`, and `bun.lock`).

Gitleaks scans the full Git history with the default rule set. This repository
is under a personal GitHub account, so `GITLEAKS_LICENSE` is not required; add
that secret if the repository is moved to an organization.

The Phase20 prerequisite is historical and complete. Reversible release
preparation and any publication decision now follow the [release checklist](docs/RELEASE.md);
completing a Phase or making the manifest publishable is not approval to upload.

## Release Convention

Use annotated tags named `vMAJOR.MINOR.PATCH` for stable releases. The
published GitHub prerelease is [v0.1.0-pre.2](https://github.com/albert-einshutoin/qzt/releases/tag/v0.1.0-pre.2);
later main changes are not in that binary. Core is a release candidate; QZI
search and the overall product remain a technical preview. Publication is a
separate owner-approved operation under the release checklist.
