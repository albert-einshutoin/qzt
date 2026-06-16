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

## Security Scans

CI runs Semgrep CE and Gitleaks on pull requests, pushes, scheduled scans, and
manual dispatches.

Semgrep uses `semgrep scan --config p/rust --error` so findings fail the job.
Tune the scan by changing the ruleset, adding a `.semgrepignore`, or filtering
with Semgrep severity levels (`INFO`, `WARNING`, `ERROR`) after the first
baseline is reviewed.

Gitleaks scans the full Git history with the default rule set. This repository
is under a personal GitHub account, so `GITLEAKS_LICENSE` is not required; add
that secret if the repository is moved to an organization.

`cargo publish` and crates.io publish dry-runs are deferred until after Phase20
stabilizes the public API.

## Release Convention

Use annotated tags named `vMAJOR.MINOR.PATCH`. The first public line is
`v0.1.0` and remains a technical preview until Product Completeness Track
Phase14-Phase23 are complete.
