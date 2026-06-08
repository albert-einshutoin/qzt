# Phase20: Public API Stabilization

## Purpose

Make QZT a stable dependency for embedders such as Memory Pager. Today
`src/lib.rs` exposes all fifteen modules as `pub mod`, including internal ones
(`cbor`, `primitives`, `fixed`, `schema`, `skeleton`), so every internal
refactor is a breaking change for downstream consumers. The writer exposes
eight near-duplicate `pack_bytes_*` free functions, and the crate-level
documentation still describes Phase0 placeholders. There is no documented
API-stability guarantee.

This phase curates the public surface, consolidates the writer entry points,
documents the public API, and adds a guard against accidental surface growth.
It MUST NOT change the container format bytes or the
`export(pack(input)) == input` invariant.

## Minimum MVP

```text
- internal-only modules (cbor, primitives, fixed, schema, skeleton) become pub(crate) or are hidden behind a curated surface
- the public API is re-exported from lib.rs as a small, intentional set (pub use)
- the eight pack_bytes_* functions are consolidated behind a writer builder
- lib.rs crate-level documentation describes the real public API, not the Phase0 placeholder note
```

## Goal MVP

```text
- the public surface carries #![warn(missing_docs)] and every public item is documented
- docs.rs metadata is configured and cargo doc builds clean
- a documented semantic-versioning and API-stability policy states what is public/stable versus internal
- a public-API snapshot test (committed surface listing or cargo-public-api) fails on unintended surface changes
- thin deprecated shims keep existing callers working for one release where needed
```

## Spec refs

```text
- No format spec section. References tasks/README.md Rust Style visibility guidance.
- This phase changes the crate's Rust API surface only, never the container format.
```

## Conformance Tests Covered

```text
- none directly; this phase protects downstream consumers from accidental breaking changes
- the writer builder reproduces every former pack_bytes_* output byte-for-byte
```

## TDD Plan

Write failing tests and checks:

```text
- a public-API snapshot test fails when an unintended item becomes public
- the writer builder produces byte-identical output to each former pack_bytes_* variant
- internal types (cbor, primitives, fixed, schema, skeleton) are no longer nameable from outside the crate
- cargo doc --no-deps produces zero warnings with missing_docs enabled
- the CLI compiles using only the curated public API
```

## Implementation Tasks

```text
1. classify each module and type as public API or internal
2. make internal modules pub(crate) and re-export the curated public types from lib.rs
3. design a WriterBuilder that consolidates the pack_bytes_* variants over WriterOptions
4. keep thin deprecated wrappers for one release if external callers exist
5. add #![warn(missing_docs)] and document every public item
6. configure docs.rs metadata in Cargo.toml
7. add a public-API snapshot test wired into the quality gate
8. write the API-stability and semantic-versioning policy doc
9. rewrite the lib.rs crate-level documentation
10. update the CLI to consume only the public API (dogfood the surface)
```

## Rust Notes

Default to `pub(crate)` and re-export a deliberately small public surface from
`lib.rs`. The `WriterBuilder` should own a `WriterOptions` and expose
profile, dictionary mode, Dense Line Index, Document Index, and container-id
overrides through chained methods. Use a sealed pattern where a public trait
must not be implemented downstream. The CLI consuming only the public API is the
cheapest proof that the surface is actually sufficient.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec
and this phase plan before continuing.

## Self-Review Checklist

```text
- Is the public surface minimal and intentional?
- Are internal modules unreachable from outside the crate?
- Does the writer builder reproduce every former pack_bytes_* output exactly?
- Is every public item documented?
- Is there an automated guard against accidental surface growth?
- Did this phase avoid any container format byte change?
```

## Done Criteria

```text
- internal modules are pub(crate) and the curated public surface is re-exported from lib.rs
- WriterBuilder replaces the pack_bytes_* sprawl with byte-identical output
- missing_docs is enabled and all public items are documented
- docs.rs metadata configured and cargo doc builds clean
- public-API snapshot test is in the quality gate
- API-stability and semver policy doc exists
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.

Depends on: Phase14 (CI runs the doc and surface-snapshot gates). Prerequisite
for Phase21 (integration examples consume the stable public surface) and
Phase22 (the vector runner uses only the public reader API). Opens the
consumer sub-track of the Product Completeness Track.
