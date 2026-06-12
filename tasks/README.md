# QZT Implementation Tasks

This directory is the execution plan for the QZT reference implementation.

The reference implementation SHOULD be written in Rust unless a project decision changes this file first. Rust fits QZT because the format needs precise binary layout, checked arithmetic, explicit errors, streaming I/O, bounded decompression, and strong testable invariants.

## Operating Rules

Every phase uses TDD:

```text
1. write or update failing tests
2. implement the smallest behavior that passes
3. run targeted tests
4. run broader verification for touched areas
5. self-review the diff
6. perform a code review
7. perform an architecture review
8. fix review findings
9. update tasks/status.md
```

Do not mark a phase complete until tests, self-review, code review, architecture review, review fixes, and status updates are done.

## Implementation Flow

Use this loop for every meaningful change:

```text
implement -> self-review -> code review -> architecture review -> fix -> verify -> update status
```

Self-review MUST check:

```text
- Does the code implement the spec invariant directly?
- Are overflow and resource limits handled before trusting file data?
- Are errors specific enough for conformance tests?
- Are tests proving both success and corruption cases?
- Did the change preserve exact export semantics?
- Did the change avoid adding extension behavior into Core?
```



## Review Gates

Every phase MUST include both review gates before completion.

Code review checks:

```text
- parser and writer code has no hidden panics on untrusted input
- errors are specific and testable
- tests cover success, corruption, and boundary cases
- public APIs remain small and coherent
- implementation follows Rust ownership and type-safety idioms
```

Architecture review checks:

```text
- module boundaries still match the spec sections
- Core behavior is not coupled to optional extensions
- data flow preserves exact export and source-of-truth semantics
- resource limits and checked arithmetic are enforced at trust boundaries
- future phases can build on the current design without rewriting completed phases
```

If a review finds a spec ambiguity or library constraint, update both `docs/QZT_v0.1_Core_Spec.md` and the relevant `tasks/PhaseN.md` before continuing.

## Rust Style Expectations

Use language features that make the binary format safer:

```text
- newtypes for offsets, sizes, chunk IDs, line IDs, and granule IDs where useful
- Result<T, QztError> for fallible operations
- checked_add / checked_mul for all offset and size arithmetic
- TryFrom for parsed fixed binary structures
- traits for ReadAt / WriteAt behavior where it keeps tests simple
- borrowed slices for fixed-layout parsing when safe
- explicit Vec allocation limits before decompression or CBOR decode
- property tests for round-trip and checked arithmetic
- golden fixtures for conformance files
```

Avoid hidden panics in parser, verifier, and reader paths. A corrupt file is an expected input, not an exceptional programming state.

## Phase File Contract

Each `PhaseN.md` contains:

```text
- Purpose
- Minimum MVP
- Goal MVP
- TDD plan
- Implementation tasks
- Self-review checklist
- Done criteria
- Status
```

Minimum MVP is the smallest useful increment that should land first.
Goal MVP is the phase's intended stopping point before the next phase starts.

## Status Tracking

`tasks/status.md` is the single progress summary.

When work starts or finishes:

```text
- update the phase state
- record the current commit if relevant
- record tests run
- record open blockers
- keep the next action concrete
```

## Phase Order

```text
Phase0  Project foundation and quality gates
Phase1  Deterministic CBOR, primitives, and errors
Phase2  Header, footer trailer, and physical ranges
Phase3  Metadata, footer payload, index root, and chunk table skeleton
Phase4  UTF-8 chunker and sparse Chunk Table writer
Phase5  No-dictionary zstd writer and finish
Phase6  Reader open/info/export and verification levels
Phase7  Sparse line index, range reads, and CLI access
Phase8  Dictionaries, resource limits, and Reader Core completion
Phase9  Core conformance hardening and release readiness
Phase10 Dense Line Index, Document Index, memory profile, and maintenance command scoping
Phase11 Search granules and raw token index MVP
Phase12 N-gram index, planner, and benchmark reporting
Phase13 Search sidecar and high-performance search goal MVP
```

Do not start Search Extension implementation before Core conformance is stable, except for design-only work.

Optional indexes and extension profiles MUST NOT block Core release readiness unless a phase explicitly says the release target includes them.

## Product Completeness Track (post-v0.1)

Phase0-Phase13 deliver a format-complete v0.1 reference implementation. The
Product Completeness Track raises maturity from "reference implementation /
technical preview" toward the spec's product goal: the Cold Evidence Container
embedded by Memory Pager and AI memory systems. These phases are process,
scalability, and integration work; none of them change the container format
bytes or the `export(pack(input)) == input` invariant.

The track has two sub-tracks. The engine sub-track makes the I/O model and
hardening production-credible. The consumer sub-track makes QZT a stable,
verifiable dependency that an external system can embed.

Engine sub-track:

```text
Phase14 Open-source release hygiene: LICENSE, CI, package metadata, contributor docs
Phase15 File-backed seeking reader (QztFileReader): bounded-memory open/range/line/export over file paths
Phase16 Streaming verification and export: remove O(file size) memory in verify_deep and export
Phase17 Streaming writer (QztFileWriter): build containers larger than RAM, byte-identical to pack_bytes
Phase18 Competitive benchmark harness: QZT vs raw zstd, seekable zstd, SQLite FTS5, ripgrep
Phase19 Resource governance and large-input hardening: wire ResourceLimits into CBOR, cap search results, extend fuzzing
```

Consumer sub-track:

```text
Phase20 Public API stabilization: curate lib.rs surface, writer builder, missing_docs, semver/stability policy, surface snapshot test
Phase21 Verified evidence retrieval and Memory Pager integration: read_document / read_range_verified / read_document_verified, evidence_ref example, concurrent verified reads
Phase22 Portable conformance vectors and format stability: golden .qzt vectors, vector runner, third-party verification, frozen v0.1 format-stability statement
```

Validation (cross-cutting):

```text
Phase23 Acceptance threshold harness: deterministic C1-C6 corpora, HARD invariants asserted, SOFT targets band-checked (see docs/QZT_v0.1_Validation_Corpus.md)
```

`docs/QZT_v0.1_Validation_Corpus.md` defines what text QZT is validated against
and what result counts as meeting expectations (HARD invariants vs SOFT target
bands). Phase23 makes that doc executable and owns the corpus generators that
Phase18 and Phase22 reuse.

Dependency order:

```text
Phase14 -> independent, land first
Phase15 -> foundation for Phase16, Phase17, Phase18, Phase21, Phase23a
Phase16 -> depends on Phase15
Phase17 -> depends on Phase15
Phase18 -> depends on Phase15 and Phase23a (shared corpora)
Phase19 -> depends on Phase15 and Phase17
Phase20 -> depends on Phase14; prerequisite for Phase21 and Phase22
Phase21 -> depends on Phase15 and Phase20; adds the base read_document API and concurrent verified reads
Phase22 -> depends on Phase20, Phase23a, and the Phase9 conformance map
Phase23a -> depends on Phase15; builds C1-C6 generators and HARD invariants that do not require evidence APIs
Phase23b -> depends on Phase21; adds C1 evidence-retrieval invariants to the same harness
```

Recommended sequence: Phase14, then Phase15, then the sub-tracks proceed in
parallel. Engine: Phase16 and Phase17 in either order, then Phase18 (after
Phase23a corpora exist), then Phase19. Consumer: Phase20, then Phase21, then
Phase22. Validation: Phase23a right after Phase15; Phase23b extends it once
Phase21 lands.

These phases MUST NOT change container format bytes. Any change that would
alter the byte layout belongs in a new format version, not in this track.

## Post-Phase23 Execution (post-v0.1 roadmaps)

Phase0-Phase23 are complete. Execution continues on two GitHub-issue
roadmaps: the refactoring roadmap (issue #31, issues #2-#30) and the product
value roadmap (issue #47, issues #33-#46). Their cross-track ordering, wave
plan, parallelism constraints, milestones, and release gates are fixed in
[PostPhase23.md](PostPhase23.md). Detailed steps and acceptance criteria live
on the issues; the rules in this README (TDD loop, review gates, no format
byte changes) continue to apply to every issue PR.
