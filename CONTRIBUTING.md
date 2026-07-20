# Contributing

QZT is developed with GitHub Flow on short-lived feature branches. Keep changes
small, reviewable, and tied to the phase plan in `tasks/`.

## Development Contract

Every implementation change follows:

```text
implement -> self-review -> code review -> architecture review -> fix -> verify -> update status
```

Do not mark a phase complete until tests, two self-review passes, code review,
architecture review, review fixes, and `tasks/status.md` updates are complete.

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

Changes to parsing, verification, or fuzz targets should also run the bounded
nightly smoke locally when `cargo-fuzz` is available:

```sh
cargo +nightly fuzz run open_verify -- -max_total_time=60 -timeout=10 -max_len=4096
```

The same fuzz command runs weekly and on manual dispatch, not on every pull
request. Generated corpus and crash state remain local; CI retains crash
artifacts for seven days when a run fails.

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
Tune the scan by changing the ruleset, adding a `.semgrepignore`, or filtering
with Semgrep severity levels (`INFO`, `WARNING`, `ERROR`) after the first
baseline is reviewed. The Semgrep container image is pinned; update it
deliberately when refreshing the security toolchain.

OSV Scanner checks `Cargo.lock` for known dependency vulnerabilities and fails
on reported vulnerabilities. This covers Rust dependency SCA; OWASP CVE Lite
CLI is intentionally not part of this workflow because it is focused on
JavaScript and TypeScript lockfiles (`package-lock.json`, `pnpm-lock.yaml`,
`yarn.lock`, and `bun.lock`).

Gitleaks scans the full Git history with the default rule set. This repository
is under a personal GitHub account, so `GITLEAKS_LICENSE` is not required; add
that secret if the repository is moved to an organization.

`cargo publish` and crates.io publish dry-runs are deferred until after Phase20
stabilizes the public API.

## Release Convention

Use annotated tags named `vMAJOR.MINOR.PATCH`. The first public line is
`v0.1.0` and remains a technical preview until Product Completeness Track
Phase14-Phase23 are complete.
