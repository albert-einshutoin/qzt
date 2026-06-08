# Phase23: Acceptance Threshold Harness

## Purpose

Turn `docs/QZT_v0.1_Validation_Corpus.md` into an executable gate. Generate the
C1-C6 validation corpora deterministically, assert the non-evidence HARD
invariants as tests, and record the SOFT targets as provisional expectation
bands so regressions are visible. This harness owns the corpus generators that
Phase18 (competitive timing) and Phase22 (golden vectors) reuse. Evidence
retrieval is added as a Phase23b extension after Phase21 delivers the verified
evidence API.

This phase adds measurement and acceptance criteria. It MUST NOT change the
container format bytes or any reader/writer behavior to make a threshold pass.

## Minimum MVP

```text
- deterministic, seeded generators for corpora C1 through C6 (see docs/QZT_v0.1_Validation_Corpus.md)
- Phase23a HARD invariants asserted per corpus: lossless round-trip, range-restore byte bound, deep-verify corruption detection
- a recorded report of the provisional SOFT targets (compression ratio, search decode ratio, peak memory) per corpus
```

## Goal MVP

```text
- SOFT targets are compared against the documented expectation bands; out-of-band results are flagged with a clear message, not silently passed and not hard-failed
- Phase23b evidence-retrieval HARD invariants (clean verified 100%, tampered failure 100%) are asserted on C1 after Phase21
- small corpus sizes run HARD invariants inside the default quality gate; large sizes run behind an opt-in flag so the default gate stays fast
- the report format is stable enough to compare across runs
```

## Spec refs

```text
- docs/QZT_v0.1_Validation_Corpus.md corpus taxonomy and acceptance thresholds
- Section 35 test suite
- Section on verification levels
```

## Conformance Tests Covered

```text
- lossless round-trip holds for every corpus C1-C6
- range restore decodes within the documented byte bound for each corpus
- a corruption sweep detects 100% of single-byte corruptions with the correct error
- Phase23b: evidence retrieval verifies clean reads and fails closed on tampered reads after Phase21
- SOFT metrics are recorded and band-checked for each corpus
```

## TDD Plan

Write failing tests:

```text
- each corpus generator is deterministic: same seed yields byte-identical corpus
- export(pack(corpus)) == corpus for every corpus C1-C6
- a range restore decodes no more than requested_size + 2 * chunk_size for each corpus
- a corruption sweep over chunk, metadata, and index bytes is detected 100% with the documented error
- Phase23b: a clean evidence read verifies and a tampered evidence read fails closed after Phase21
- a SOFT metric outside its expectation band is flagged (not hard-failed) with a clear message
```

## Implementation Tasks

```text
1. build deterministic seeded generators for C1-C6
2. assert the Phase23a HARD invariants per corpus (round-trip, range bound, corruption detection)
3. measure the SOFT targets per corpus (compression ratio, search decode ratio, peak memory)
4. compare SOFT targets against the documented provisional bands and flag out-of-band results
5. emit a stable, comparable report
6. wire small-size HARD invariants into make check; gate large-size runs behind an opt-in flag
7. expose the generators as a shared module reused by Phase18 and Phase22
8. after Phase21, add Phase23b C1 evidence clean/tampered invariants to the same harness
```

## Rust Notes

Reuse the writer and reader rather than reimplementing pack or export. Keep
seeds explicit so corpora regenerate byte-identically. Keep small-corpus HARD
invariants in the default gate; put large-corpus and timing-sensitive runs
behind the same opt-in mechanism Phase18 uses so the default `make check` stays
fast. SOFT targets are recorded evidence: out-of-band is a flag and an
investigation prompt, not an automatic build failure.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec
and this phase plan before continuing.

## Self-Review Checklist

```text
- Are all six corpora deterministic and regenerable byte-identically?
- Do the HARD invariants hold on every corpus?
- Are SOFT targets band-checked and flagged as provisional, not silently passed?
- Do small-corpus HARD invariants run in the default gate without slowing it materially?
- Are the generators shared with Phase18 and Phase22 rather than duplicated?
- Did this phase avoid any format byte change and any behavior change made just to pass a threshold?
```

## Done Criteria

```text
- deterministic C1-C6 generators exist and are shared
- HARD invariant tests pass for every corpus
- corruption sweep detection is 100%
- Phase23b evidence clean/tampered invariants pass after Phase21
- SOFT targets are recorded and band-checked against docs/QZT_v0.1_Validation_Corpus.md as provisional bands
- small-size HARD invariants run in make check; large-size runs are opt-in
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.

Depends on: Phase15 for Phase23a (file-backed reader for the peak-memory and
seek bounds). Phase23b depends on the evidence-retrieval API from Phase21 for
the evidence invariants on C1. Provides corpora to Phase18 and Phase22. Run
Phase23a right after Phase15 so the acceptance thresholds are in place before
the competitive and vector work consumes the same corpora.
