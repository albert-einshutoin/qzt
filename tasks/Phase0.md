# Phase0: Project Foundation and Quality Gates

## Purpose

Create a repeatable Rust project foundation before implementing the QZT format.

## Minimum MVP

```text
- Rust workspace or single crate initialized
- CLI crate name reserved as qzt
- library crate exposes placeholder modules
- test harness runs
- formatting and lint commands documented
```

## Goal MVP

```text
- local quality command runs fmt, clippy, and tests
- fixture directories exist
- corruption fixture strategy is documented
- status.md is updated after every implementation change
```

## Spec refs

```text
- Section 36.1 Cut 0: format foundation
- Section 35 Test suite
```

## Conformance Tests Covered

```text
- none directly; this phase creates the harness required to reproduce all later conformance tests
```

## TDD Plan

Start with tests that assert the project harness is wired:

```text
- smoke test can import the library
- CLI binary responds to --help once CLI exists
- fixture directory discovery test passes
```

## Implementation Tasks

```text
1. create Cargo workspace
2. create library module skeleton
3. create CLI crate or binary target
4. create tests/fixtures directory layout
5. add local scripts or Makefile commands
6. document commands in README or status.md
```

## Rust Notes

Prefer a library-first design. The CLI should call library APIs rather than owning format logic.

## Self-Review Checklist

```text
- Can a new contributor run all checks with one command?
- Are generated/build artifacts ignored?
- Is the crate structure compatible with future fuzz/property tests?
- Did status.md get updated?
```

## Done Criteria

```text
- cargo fmt passes
- cargo test passes
- cargo clippy passes or has documented temporary gaps
- tasks/status.md marks Phase0 complete
```

## Status

Pending.
