# Phase19: Resource Governance and Large-Input Hardening

## Purpose

Close the remaining trust-boundary gaps so the format is safe on adversarial
and very large inputs. The CBOR decoder currently enforces hardcoded allocation
limits (`MAX_PHASE1_ALLOCATION`, `MAX_PHASE1_ITEMS`) that are disconnected from
`ResourceLimits`, so a caller's custom limits never reach CBOR decoding. The
search planner does not enforce `max_search_results`. Fuzz coverage is a
deterministic smoke harness, not a longer-running campaign.

This phase wires resource limits through every decode path, enforces search
result caps, documents per-operation peak-memory guarantees, and extends
fuzzing to large and streaming inputs.

This phase MUST NOT change format bytes. It tightens enforcement and
documentation around existing structures.

## Minimum MVP

```text
- CBOR decode accepts an allocation/items budget sourced from ResourceLimits, replacing the hardcoded constants
- open_with_limits propagates that budget into CBOR validation
- the search planner enforces max_search_results (Section 33)
```

## Goal MVP

```text
- a cargo-fuzz target for open + verify, beyond the Phase9 deterministic smoke
- large-input property tests: streaming writer/reader round-trip on inputs exceeding an in-memory budget
- per-operation peak-memory guarantees documented for open, range, line, verify (quick/normal/deep), and search
- adversarial fixtures: oversized declared sizes, deep CBOR nesting, very many chunks, very many dictionaries
```

## Spec refs

```text
- Section 23 error taxonomy
- Section 33 search limits, including max_search_results
- Section on resource limits and bounded decompression
- Section 9.1 reader open trust boundary
```

## Conformance Tests Covered

```text
- a custom ResourceLimits allocation budget rejects an oversized CBOR block before allocation
- max_search_results caps the returned hits and marks the report as capped
- oversized declared sizes are rejected with specific errors before allocation
- deeply nested or oversized CBOR is rejected without stack or heap blowup
- streaming round-trip holds on inputs larger than the in-memory budget
```

## TDD Plan

Write failing tests:

```text
- open_with_limits with a small allocation budget rejects an otherwise-valid large CBOR block
- the hardcoded CBOR constants no longer gate behavior; the limit comes from ResourceLimits
- a search returning more than max_search_results is capped and flagged
- an adversarial container declaring a huge uncompressed size is rejected before allocation
- a deeply nested CBOR input is rejected with a specific error, never a panic or unbounded recursion
- a streaming write-then-read round-trip succeeds on an input exceeding the configured in-memory budget
```

## Implementation Tasks

```text
1. thread a budget from ResourceLimits into the CBOR decoder, removing MAX_PHASE1_ALLOCATION / MAX_PHASE1_ITEMS as the source of truth
2. propagate limits through open_with_limits into every CBOR validation call
3. enforce max_search_results in the planner and reflect capping in SearchReport
4. add adversarial fixtures for oversized sizes, deep nesting, many chunks, and many dictionaries
5. add a cargo-fuzz target for open + verify
6. add large-input property tests using the streaming writer/reader
7. document per-operation peak-memory guarantees
```

## Rust Notes

Bound CBOR allocation and recursion depth from `ResourceLimits` so a caller can
tighten or loosen limits without recompiling. Every size or count read from the
file must be checked against the budget before it drives an allocation. Keep
the fuzz target deterministic-seedable for CI reproduction while allowing longer
local runs. Reuse Phase15/Phase17 streaming paths so large-input tests do not
require holding the whole input in memory.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec
and this phase plan before continuing.

## Self-Review Checklist

```text
- Do caller-supplied ResourceLimits actually reach CBOR allocation?
- Is every file-derived size checked against a budget before allocation?
- Is max_search_results enforced and surfaced as capped?
- Does adversarial input fail with specific errors and never panic?
- Are per-operation peak-memory guarantees documented and tested?
- Did this phase avoid any format byte change?
```

## Done Criteria

```text
- ResourceLimits-driven CBOR allocation budget implemented and tested
- max_search_results enforcement implemented and tested
- adversarial fixtures pass (rejected with specific errors, no panic)
- cargo-fuzz open+verify target exists
- large-input streaming round-trip property tests pass
- per-operation peak-memory guarantees documented
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Complete.

Completed on: 2026-06-08

Implementation scope:

```text
- Threaded ResourceLimits into deterministic CBOR validation and schema block decode.
- Added SearchOptions max_search_results cap.
- Documented memory guarantees and added cargo-fuzz open+verify target.
```

Verification:

```text
- cargo test --test phase19_resource_governance
- make check
```

Review notes:

```text
- Self-review pass 1 completed: caller-supplied CBOR allocation budgets reject otherwise-valid oversized blocks before allocation.
- Self-review pass 2 completed: search result caps stop result growth and mark reports capped.
- Code review completed: resource limit errors use existing typed QztError paths and are covered by regression tests.
- Architecture review completed: large-input hardening is centralized in ResourceLimits/SearchOptions and reused by reader/search paths.
```

Depends on: Phase15 and Phase17 (streaming paths enable large-input tests
without full buffering). Resolves the M-1 CBOR-limits-wiring follow-up and adds
the search result cap and adversarial hardening.
