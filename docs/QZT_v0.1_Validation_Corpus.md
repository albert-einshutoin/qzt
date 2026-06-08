# QZT v0.1 Validation Corpus and Acceptance Thresholds

Date: 2026-06-08

## Purpose

Define what text QZT is validated against and what result counts as "meeting
expectations". The release hardening note deliberately records metrics without
an SLA. This note adds the missing layer: a corpus taxonomy and expectation
bands so a result can be judged pass or investigate.

Two kinds of criteria are used:

```text
- HARD invariant (MUST): holds on every corpus, every run. A violation is a release blocker.
- SOFT target (SHOULD): an expectation band. Inside the band is "meets expectations". Outside is investigate, not an automatic block.
```

These thresholds are product expectations, not a guaranteed SLA. They exist so
the project can tell whether a change improved or regressed product behavior.

## Corpus Taxonomy

QZT stores evidence for AI memory systems. The validation corpus models the
text such systems actually retain and cite.

```text
C1 conversation transcripts
   multi-turn dialogue, markdown-ish, moderate redundancy, UTF-8 with code blocks
   primary use case (spec Section 3 example); doc_id is a conversation; evidence_ref round-trip

C2 application and server logs
   timestamped repeated lines, ASCII-dominant, high redundancy
   high compression, bounded line access, rare-token search narrowing

C3 prose documents and knowledge base
   natural language, low redundancy, long paragraphs, markdown
   UTF-8 boundaries, doc_id ranges, n-gram substring search

C4 source code and structured data
   code, JSON, CSV; many short tokens; moderate redundancy
   byte-exact round-trip (whitespace and indentation preserved), token search

C5 multilingual / CJK / emoji
   Japanese, Chinese, emoji; multi-byte UTF-8 throughout
   100% UTF-8 boundary safety, Unicode-scalar n-gram unit, boundary-safe range reads

C6 adversarial / pathological
   huge single line (no newlines), near-random (incompressible), extreme repetition, mixed CRLF/LF
   no panic, exact round-trip, incompressible upper bound, resource-limit enforcement
```

## Capability Expectations

For every corpus, each QZT capability has a HARD invariant and, where relevant,
a SOFT target.

```text
lossless round-trip
  HARD: export(pack(x)) == x, byte-exact, all corpora
  SOFT: none (binary, always)

compression ratio
  HARD: never catastrophically worse than whole-file zstd
  SOFT: provisional target band: container overhead within +3..5% of whole-file zstd; C2 < 15% of source; C3 < 45% of source

range restore
  HARD: returns exactly the requested bytes
  SOFT: provisional target band: decoded_bytes <= requested_size + 2 * chunk_size, independent of file size

line access
  HARD: correct line bytes including spanning lines
  SOFT: provisional target band: reads bounded by the chunks the line spans; dense index gives O(1) offset

rare-token search
  HARD: hit set equals ground truth
  SOFT: provisional target band: candidate decoded_bytes / source_bytes < ~1%

common-token search
  HARD: hit set equals ground truth
  SOFT: provisional target band: capped before full candidate decode

deep verify
  HARD: detects single-byte corruption in chunk / metadata / index with the correct error
  SOFT: provisional target band: corruption sweep detection rate 100%, false-negative rate 0

evidence retrieval
  HARD: a verified read fails closed on mismatch, never returns wrong bytes
  SOFT: provisional target band: clean retrieval verified-match 100%; tampered retrieval failure 100%

search sidecar size
  HARD: rebuildable; rejection does not break Core read/export/verify
  SOFT: provisional target band: token and n-gram sidecars 1.0..1.7x source

peak memory
  HARD: no OOM on large input
  SOFT: provisional target band: range / line / verify peak <= max_chunk_size + index region, independent of file size
```

## Per-Corpus Expectation Notes

```text
C1 conversation: compression 25..45% of source; doc_id per conversation resolves; evidence_ref restore is verified.
C2 logs: compression < 15% of source; rare-token search decodes far less than a raw scan.
C3 prose: compression 30..45%; UTF-8 safe; doc_id ranges restore exactly.
C4 code/structured: compression 20..40%; whitespace and indentation byte-exact; token search correct.
C5 multilingual: round-trip exact; no split Unicode scalar at any chunk or range boundary.
C6 adversarial: no panic; incompressible data stays near 1.0x plus bounded overhead; limits enforced.
```

## Mapping to Phases

```text
- Phase23a builds the deterministic generators for C1..C6 and asserts the non-evidence HARD invariants, recording the SOFT targets as provisional expectation bands.
- Phase23b adds the evidence-retrieval HARD invariants after Phase21 provides the verified evidence API.
- Phase18 reuses the same generators for competitive timing and adds a large-size option.
- Phase21 verifies the evidence-retrieval HARD invariants (clean 100%, tampered 100%).
- Phase22 freezes a representative subset of these corpora as portable golden vectors.
```

## Reproducibility

```text
- corpora are deterministic and seeded; regeneration is byte-identical
- environment (CPU, OS, toolchain, tool versions) is captured in any timing report
- SOFT targets are recorded as evidence; being out of band is flagged, not silently passed
```

## Non-Goals

```text
- this is not a guaranteed SLA
- competitive claims belong to Phase18, not to this note
- SOFT bands are provisional and are revisited after Phase23a's first executable report and Phase18's competitive benchmark evidence
```
