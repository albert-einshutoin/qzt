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

For documentation or release-hygiene changes, also run:

```sh
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo package --allow-dirty
```

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
cargo test --test phase9_hardening -- --nocapture

make check
```

When you change `CORE_CONFORMANCE_MAP`, include `phase9_hardening` in the
focused command even if the new evidence test lives in another phase file.

## Security Scans

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
