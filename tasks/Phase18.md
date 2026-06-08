# Phase18: Competitive Benchmark Harness

## Purpose

Answer the unvalidated product question: "why QZT over existing tools?" The
release hardening note states plainly that no comparison exists against SQLite
FTS, Tantivy, Lucene, seekable zstd, or split-frame object storage. Without
that evidence, the product's adoption reason is unproven.

This phase builds a reproducible comparative benchmark on a common corpus,
measuring the operations QZT actually claims to do well: random range restore,
single-line restore, evidence-position search with original-byte verification,
and on-disk size including any search sidecar.

This phase is measurement only. It MUST NOT change the format or the reader and
writer behavior to flatter the numbers.

## Minimum MVP

```text
- reuse the Phase23a C1-C6 corpus generators (docs/QZT_v0.1_Validation_Corpus.md), adding a large option (>= 100 MB) so in-memory baselines are actually stressed
- QZT vs raw-zstd-whole-file on each corpus: random range restore latency and bytes decompressed
- a documented methodology: which corpus, environment capture, exact reproduction command
- results recorded in docs/, explicitly labeled as local evidence, not an SLA
```

## Goal MVP

```text
- QZT sidecar search vs SQLite FTS5 and vs ripgrep on the same corpus: query latency, index build time, index size ratio, and identical hit-set correctness
- QZT vs seekable zstd for random access latency
- an honest "when NOT to use QZT" section derived from the numbers, including the observed sidecar-larger-than-source result
- the benchmark is opt-in (feature flag or ignored test) so the default quality gate stays fast
```

## Spec refs

```text
- No format spec section. References docs/QZT_v0.1_Release_Hardening.md "Remaining Product Evidence Gap".
```

## Conformance Tests Covered

```text
- none directly; this phase produces external product evidence, not format conformance
- correctness cross-check: QZT search and the reference tool return the same hit set on the shared corpus
```

## TDD Plan

Write failing tests and checks:

```text
- the corpus generator is deterministic: same seed yields identical bytes
- QZT range restore returns exactly the requested bytes and matches a ground-truth slice of the corpus
- QZT search hit set equals the reference-tool hit set for a fixed query set (correctness gate before timing)
- the benchmark runner records all required metrics without panicking when an optional external tool is absent
- size-ratio reporting includes container size and sidecar size separately
```

## Implementation Tasks

```text
1. reuse the Phase23a C1-C6 corpus generators and add a large-size parameter rather than writing a separate generator
2. implement a QZT-vs-raw-zstd random range restore benchmark with bytes-decompressed accounting, per corpus class
3. add an opt-in feature flag (for example "bench-compete") gating external-tool comparisons
4. integrate SQLite FTS5 and ripgrep comparisons behind that flag, with graceful skip when the tool is missing
5. add a hit-set correctness cross-check before any timing is trusted
6. capture environment metadata (CPU, OS, toolchain, tool versions) in the report
7. record results and a "when NOT to use QZT" section in docs/
8. keep the comparison out of the default make check path
```

## Rust Notes

Invoke external tools (sqlite3, ripgrep) as documented optional system
dependencies behind a Cargo feature so CI without them still passes. Depends on
Phase15 for fair large-file QZT numbers; running these comparisons against the
in-memory reader would measure the wrong thing. Report timing as evidence with
environment capture, never as a guaranteed threshold. Keep correctness gating
ahead of timing so a faster-but-wrong result can never look like a win.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec
and this phase plan before continuing.

## Self-Review Checklist

```text
- Is the corpus deterministic and large enough to stress memory and I/O?
- Does the comparison gate correctness before timing?
- Are external-tool comparisons opt-in and skippable so the default gate stays fast?
- Is the environment captured so results are reproducible?
- Does the report honestly state where QZT loses, including sidecar size?
- Are QZT numbers taken from the file-backed reader, not the in-memory reader?
```

## Done Criteria

```text
- deterministic large-corpus generator exists
- QZT vs raw-zstd range-restore benchmark runs and records metrics
- QZT vs SQLite FTS5 and ripgrep search comparison runs behind a feature flag
- search hit-set correctness cross-check passes
- results and a "when NOT to use QZT" section are recorded in docs/
- the comparison is excluded from the default quality gate
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Complete.

Completed on: 2026-06-08

Implementation scope:

```text
- Added deterministic competitive benchmark options/reporting using Phase23 corpus generators.
- Compared QZT file-backed range restore with whole-file raw zstd decode.
- Added bench-compete feature hooks for ripgrep and SQLite FTS5 hit-count correctness.
```

Verification:

```text
- cargo test --test phase18_competitive_benchmark
- cargo test --features bench-compete --test phase18_competitive_benchmark
- make check
```

Review notes:

```text
- Self-review pass 1 completed: raw-zstd range comparison asserts byte equality before reporting timings.
- Self-review pass 2 completed: feature-gated ripgrep and SQLite FTS5 hooks skip missing tools but fail on hit-count disagreement.
- Code review completed: benchmark output records corpus, encoded sizes, decoded bytes, timings, and search correctness counts.
- Architecture review completed: external tools stay outside the default gate while preserving an opt-in correctness path for product validation.
```

Depends on: Phase15 (fair large-file QZT numbers require the file-backed
reader) and Phase23a (shared C1-C6 corpus generators and acceptance thresholds).
Closes the competitive-validation gap identified in the product assessment and
the Release Hardening note.
