# Post-Phase23 Execution Plan (post-v0.1)

[日本語](PostPhase23.ja.md)

Date: 2026-06-12

Phase0-Phase23 are complete. This document is the execution plan that follows
Phase23. The detailed work items live as GitHub issues; this document fixes
their cross-track order, the parallelism rules, the milestones, and the
release gates.

Source of truth:

```text
- Refactoring roadmap: issue #31 (24 issues, #2-#30, labels refactor / phase-1..5)
- Product value roadmap: issue #47 (14 issues, #33-#46, labels product / value-1..4)
- This file: cross-track ordering, wave plan, milestones, release gates
```

Each issue already contains steps, acceptance criteria, and pitfalls at a
granularity a junior engineer can complete alone. Do not duplicate that
content here; when scope changes, update the issue.

## Goal

Two tracks run toward one goal: ship QZT v0.1.0 as a credible technical
preview.

```text
- Refactoring track (#31): make internals single-sourced and maintainable
  (about 1,200-1,500 duplicate lines removed, 1 real bug fixed, hot-path
  allocations reduced, public API consolidated)
- Value track (#47): close the evidence loop in the CLI, externalize trust,
  remove install friction, prove the value honestly
```

Neither track changes container format bytes. Byte-layout changes belong to a
new format version, never to these tracks.

## Operating Rules (shared)

```text
1. 1 issue = 1 PR. Merge condition: make check green.
2. Refactor PRs must not change behavior; behavior-changing issues
   (#3, #8, #22) record the change in CHANGELOG.md.
3. Document only merged features. Never advertise unimplemented behavior.
4. Every command or output example written into docs or tutorials is actually
   executed first.
5. Irreversible external actions (crates.io publish, v0.1.0 tag) require
   explicit owner approval.
6. Issue line numbers were captured at commit 024c6c2; search by symbol, not
   by line number.
```

## Cross-Track Constraints

```text
- #33-#37 (CLI issues) and #25 (main.rs restructuring) must not be in flight
  at the same time. Land all of #33-#37 first, then do #25.
- #38 (pack-docs) starts after #22 (pack API consolidation) so it is not
  built on an API that is being replaced.
- #42 must not publish to crates.io before the public API is final (#22, #30)
  and the owner approves. Dry-run only until then.
- #26 (test reorganization) must leave tests/vectors/ untouched; #40 freezes
  it as a portable kit.
- #28 (Edition 2024) runs alone; never in parallel with other PRs.
- Refactor Phase 1 completes before any Phase 2+ refactor issue starts
  (#31 rule). Value Phase 1 may run in parallel with refactor Phase 1-2
  because it is CLI/docs-centric. When in doubt: format/library work belongs
  to #31, CLI/docs work to #47.
```

## Wave Plan

Waves are merge batches. Inside a wave, lanes run in parallel; an issue
starts only after the issues it depends on are merged.

### Wave 0 - in flight

```text
1. Merge PR #32 (= issue #2, QztError Copy). CI is already green.
```

### Wave 1 - refactor foundation (Refactor Phase 1 gate)

```text
Refactor lane: #8 first (real bug: WriterBuilder::pack skips
               validate_profile) -> #3 (needs #2) -> #4, #5, #6, #7 in
               parallel -> #9 last (needs #4)
Value lane:    #33 (qzt info + JSON foundation) may start here; it is the
               prerequisite for #34/#35/#36. Land #3 before #34 so the
               exit-code and error-output contract builds on the final error
               type.
Gate:          all of #2-#9 merged before any Phase 2+ refactor issue.
```

### Wave 2 - JSON evidence loop + duplicate removal

```text
Value lane:    #34, #35, #36 in parallel (all need #33), then #37
               (stdin streaming pack, independent)
Refactor lane: #10 -> #11; #12, #13, #16 in parallel; #14 (needs #3, #5);
               #15 (needs #4)
Constraint:    do not start #25 while #33-#37 are in flight.
```

### Wave 3 - trait unification

```text
Refactor lane: #17 (needs #14, #15); #18 (needs #13); #20 (needs #4);
               #21 (needs #15)
Value lane:    no new issue; finish Wave 2 leftovers.
```

### Wave 4 - API consolidation + trust externalization

```text
Refactor lane: #22 (needs #8); #19 (needs #15, #17)
Value lane:    #39 (attest; needs #33, #34); #40 (conformance vector kit;
               coordinate scope with #26)
Then:          #38 (pack-docs; needs #35 and, per constraint, #22)
```

### Wave 5 - structural unification + CLI contract

```text
Refactor lane: #23 (needs #13, #22; highest risk - golden container tests
               mandatory); #24 (needs #18, #23); #26 (needs #16; keep
               tests/vectors frozen); #25 (only after #33-#37 are all merged)
Value lane:    #41 (CLI reference and stability contract; needs V1 issues
               and #39)
```

### Wave 6 - release engineering + final polish

```text
Refactor lane: #30 (needs #7, #9, #22); #27 (search perf, measurement
               mandatory; needs #15, #19, #21); #29 (CI hardening; needs
               #26); #28 (Edition 2024; solo window)
Value lane:    #42 (crates.io dry-run and publish checklist; needs #22, #30);
               #43 (cargo-dist prebuilt binaries); #44 (README revamp; needs
               #41, #43)
```

### Wave 7 - proof and release

```text
Value lane:    #45 (competitive benchmarks vs raw zstd / ripgrep / SQLite
               FTS5 with honest reading; result feeds the #44 README);
               #46 (three end-to-end tutorials; needs #37, #38, #39)
Release gates, in order, each owner-approved:
  1. tag v0.1.0 technical preview
  2. crates.io publish (after the #42 checklist passes)
  3. GitHub Release binaries via the #43 pipeline, then announce
```

## Milestones

| Milestone | Definition of done | Issues |
|---|---|---|
| M1 Foundation clean | Refactor Phase 1 merged, real bug #8 fixed | #2-#9 (PR #32 included) |
| M2 Evidence loop closed | CLI pin -> verify -> retrieve -> prove flow with `--format json` | #33-#38 |
| M3 Internals unified | duplicate removal, trait unification, structural consolidation merged | #10-#26 |
| M4 Trust externalized | attest, 14+ conformance vectors, CLI stability contract | #39-#41 |
| M5 Distributable | crates.io ready, prebuilt binaries, README revamp, polish done | #27-#30, #42-#44 |
| M6 v0.1.0 technical preview shipped | benchmarks and tutorials published, owner-approved tag and publish | #45, #46 + release gates |

## Deferred Beyond These Tracks (v0.2 candidates)

Known limitations that neither roadmap closes. Each needs a format or
extension version decision and a new owner decision before work starts.

```text
- sidecar build memory: index construction still holds the posting map in
  memory (~0.6-1.3 GB on a 42 MB corpus)
- sidecar size: QZI is an uncompressed MVP structure (~2.1x source on a
  realistic log corpus)
- normalized search: SearchIndexSource::NormalizedUtf8 (Unicode
  normalization, case folding, width folding)
- phrase / positional search semantics (current multi-token search is
  co-occurrence)
- maintenance commands: repack / merge / compact
- embedded qzt-search-block-v1
- competitive comparisons beyond #45 scope: Tantivy, Lucene, seekable zstd,
  split zstd frames
```

## Status Tracking

Per-issue progress is tracked on the GitHub issues themselves (checklists in
#31 and #47). `tasks/status.md` keeps only the track-level summary and the
concrete next action.
