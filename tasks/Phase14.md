# Phase14: Open-Source Release Hygiene

## Purpose

Make the repository legally and operationally ready for public release. The
format and reference implementation are spec-complete, but the repository
cannot be adopted, redistributed, or trusted externally without a license,
automated verification, and contributor entry points.

This phase changes repository process and metadata only. It MUST NOT change
any container format behavior, byte layout, or `export(pack(input)) == input`
semantics.

## Minimum MVP

```text
- LICENSE present (dual MIT OR Apache-2.0, the Rust ecosystem default)
- Cargo.toml carries public package metadata (license, description, repository, readme, keywords, categories, rust-version)
- GitHub Actions workflow runs `make check` on push and pull_request
- .github directory structure exists
```

## Goal MVP

```text
- CONTRIBUTING.md restates the TDD + dual review-gate operating rules from tasks/README.md
- SECURITY.md documents a private disclosure contact and supported-version policy
- CI runs a toolchain matrix: stable plus a pinned MSRV
- CI builds documentation with `cargo doc` and fails on doc warnings
- `cargo package --allow-dirty` or an equivalent packageability check passes
- crates.io publish dry-run is explicitly deferred until after Phase20 stabilizes the public API
- release tagging and CHANGELOG conventions are documented
```

## Spec refs

```text
- No format spec section. References tasks/README.md operating rules and the Makefile quality gate.
```

## Conformance Tests Covered

```text
- none directly; this phase guarantees that every existing conformance test runs automatically on every push
```

## TDD Plan

CI and metadata are validated by reproducible commands rather than Rust unit
tests:

```text
- `cargo package --allow-dirty` succeeds with no missing-metadata errors
- `cargo doc --no-deps` produces zero warnings
- a pinned-MSRV `cargo build` succeeds
- the CI workflow file is valid and runs `make check` to green on a clean checkout
- a license-presence check (file exists and is referenced by Cargo.toml) passes
```

## Implementation Tasks

```text
1. add LICENSE-MIT and LICENSE-APACHE, set license = "MIT OR Apache-2.0" in Cargo.toml
2. fill Cargo.toml package metadata: description, repository, readme, keywords, categories, rust-version (MSRV >= 1.87 due to u64::is_multiple_of)
3. add .github/workflows/ci.yml running fmt, clippy -D warnings, and test on stable and MSRV
4. add a cargo doc job that treats warnings as errors
5. add CONTRIBUTING.md pointing to the phase contract and review gates
6. add SECURITY.md with disclosure contact and supported versions
7. add CHANGELOG.md seeded with the v0.1 technical preview entry
8. verify `cargo package --allow-dirty`
9. document that crates.io publish dry-run is deferred until after Phase20 public API stabilization
10. document the release-tag convention in CONTRIBUTING.md or README
```

## Rust Notes

Pin the MSRV explicitly and test it in CI so the minimal-dependency promise
(blake3, zstd, proptest) stays verifiable. Note that the current code uses
`u64::is_multiple_of` (stabilized in Rust 1.87), so the MSRV is modern: minimal
dependencies do not imply an old-toolchain build. Set `rust-version` to at least
1.87 (or refactor the single call site if an older MSRV is required) and assert
it in the CI matrix. Do not add new runtime dependencies in this phase.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec
and this phase plan before continuing.

## Self-Review Checklist

```text
- Can an external user legally fork and redistribute the repository?
- Does CI reproduce the exact local `make check` gate?
- Is the MSRV both declared and tested?
- Are there zero new runtime dependencies?
- Does the packageability check prove the crate metadata and included files are coherent?
- Is crates.io publish explicitly gated on Phase20 public API stabilization?
- Is the technical-preview status still clearly stated, not overstated as production-ready?
```

## Done Criteria

```text
- LICENSE files exist and Cargo.toml license metadata matches
- CI runs make check green on push and pull_request
- MSRV build job passes
- cargo doc job passes with no warnings
- cargo package or equivalent packageability check passes
- crates.io publish dry-run is deferred to the post-Phase20 release gate
- CONTRIBUTING.md and SECURITY.md exist
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.

Depends on: none. This is the cheapest, highest-leverage product-readiness
phase and should land first in the Product Completeness Track.
