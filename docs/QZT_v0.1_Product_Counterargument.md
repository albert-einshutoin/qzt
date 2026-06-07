# QZT v0.1 Product Counterargument

Date: 2026-06-07
Status: Adversarial critique

This document argues against the current QZT product specification and phase plan.
It is intentionally one-sided. Its job is not to be fair, but to describe why QZT may not deserve to exist even if the current Core implementation is technically correct.

Targets:

- [QZT v0.1 Core Specification](QZT_v0.1_Core_Spec.md)
- [QZT v0.1 Core Readiness](QZT_v0.1_Core_Readiness.md)
- [Phase plan](../tasks/README.md)
- [Task status](../tasks/status.md)
- [Phase10](../tasks/Phase10.md), [Phase11](../tasks/Phase11.md), [Phase12](../tasks/Phase12.md), [Phase13](../tasks/Phase13.md)

## Counter-thesis

QZT is probably implementable as a binary container. That is not the hard question.

The hard question is whether a new file format is the right product. The strongest counterargument is:

```text
QZT solves a format problem, but the valuable user problem is retrieval, provenance, search, and workflow integration.
If those valuable layers live outside Core, the Core format becomes a well-tested but low-demand storage primitive.
```

In other words, the project can succeed at conformance and still fail as a product.

## Why building QZT may not matter

### 1. The primary value is outside Core

The current Core promise is lossless export, chunked zstd storage, byte range access, line access, and verification. These are real engineering properties, but they are not the workflow most users buy.

For AI memory, legal evidence, log analysis, or archival retrieval, users usually need:

- ingestion from existing sources
- document identity
- provenance metadata
- semantic or lexical search
- ranking
- access control
- retention policy
- UI or API integration
- migration from existing data stores

The spec explicitly separates QZT from Memory Pager and excludes semantic search, vector database behavior, summarization, ranking, and mandatory token/ngram search from Core. That keeps Core clean, but it also removes most of the product value from the thing being released first.

Core can therefore become an implementation milestone that does not answer why anyone would adopt the format.

### 2. "Queryable" is weaker than the name implies

The name says Queryable Zstd Text Container, but Core queries are byte and line reads. Search is optional. Semantic search is external. Token and n-gram indexes are later extensions.

This creates a product expectation mismatch:

```text
User expectation: I can query compressed text.
Core reality: I can read byte ranges and lines if I already know where to look.
```

Byte and line addressing are useful for evidence replay, but they are not discovery. If discovery is delegated to a sidecar, external search engine, or Memory Pager, QZT is not the query system. It is the object the query system points into.

That may be too small a product surface to justify a new format.

### 3. Existing tools may be good enough

QZT must compete against lower-adoption-cost combinations such as:

- `.zst` files plus an offsets manifest
- split zstd frames in a directory or object store
- SQLite plus FTS and BLOB chunks
- Tantivy, Lucene, Meilisearch, or Elasticsearch plus stored source offsets
- Parquet/Arrow for structured text logs
- tar-like archives plus checksummed manifests
- content-addressed blobs plus external search indexes

Those alternatives are not identical to QZT. The counterargument is that they may be operationally sufficient while being easier to adopt.

QZT asks users to accept a new binary format, new CLI, new reader library, new conformance rules, new sidecar semantics, and new debugging tools. Unless QZT produces an obvious advantage, existing stacks win by default.

### 4. The format is intentionally not `.zst`

The spec says QZT is not a `.zst` stream. Standard zstd tools cannot decode a whole `.qzt` file directly.

That decision is understandable for container structure, but it creates a durable adoption cost:

- existing backup, inspection, and decompression tools do not understand QZT
- users need a QZT reader to recover data
- operational teams must trust a young format for long-term evidence
- debugging corrupt files requires QZT-specific knowledge

For a cold evidence container, long-term recoverability matters. A format that cannot be read by the dominant decompression tool has to prove that its index and verification benefits are worth the interoperability loss.

### 5. Evidence refs are not enough without a source workflow

The spec defines evidence refs into original text ranges. That is useful only if an external system reliably creates, stores, migrates, and validates those refs.

The hard parts are not just offsets:

- what is a document?
- how are document boundaries detected?
- how are source identities preserved across repacks?
- how are deleted or redacted sources represented?
- how are byte ranges mapped to user-visible excerpts?
- how are normalized search hits proven against original bytes?
- how does an LLM memory system keep refs stable across ingestion changes?

QZT can verify that referenced bytes still exist in a container. It does not, by itself, prove that the right bytes were referenced, that the source was complete, or that the memory system used the evidence correctly.

That weakens the claim that QZT is "evidence-native" as a product rather than as a storage substrate.

### 6. Immutability conflicts with living datasets

QZT Core is immutable after `finish()`. Updates require a new file or future maintenance operations such as repack, merge, or compact.

This is clean for archival evidence. It is awkward for active memory systems, append-heavy logs, evolving document corpora, and user data that needs deletion or redaction.

The likely operational result is a second system around QZT:

- append logs before packing
- sidecar indexes after packing
- tombstone or redaction metadata outside Core
- merge/compact jobs
- cache invalidation
- object-store lifecycle rules

Once that surrounding system exists, QZT may no longer be the main product. It becomes one internal storage artifact among several.

### 7. The sidecar may become the real product

The current plan puts high-performance search in Phase13 through a `.qzi` sidecar. That sidecar contains the hot, rebuildable, memory-mappable index.

If the sidecar is where search performance lives, then the core question becomes:

```text
Why must the sidecar point to a .qzt file instead of raw files, split zstd frames, content-addressed chunks, SQLite rows, or object-store blobs?
```

If the answer is only "because QZT has stable checksummed ranges", that may be valuable for some evidence workflows, but it is not enough for broad product pull.

The risk is that Phase13 proves a good sidecar design while accidentally proving that QZT Core is replaceable.

### 8. Verification may be overbuilt for the median user

QZT has strong verification levels, deterministic CBOR constraints, checksums, strict offsets, and corruption handling. These are good engineering properties.

The product risk is that many users do not need this much proof. They may accept:

- object-store checksums
- database checksums
- zstd frame checksums
- periodic backup validation
- search-index rebuilds from source

If the buyer is not explicitly evidence-sensitive, QZT's correctness work becomes invisible overhead. The implementation can be excellent and still fail to create demand.

### 9. Current benchmarks do not prove the product

The readiness note records smoke baselines, not release promises. That is appropriate, but it means the current evidence does not yet prove QZT is competitive.

The open product questions are:

- Is packing fast enough compared with plain zstd?
- Is compression ratio acceptable with independent frames?
- Does range access beat simpler split-file or seekable-zstd strategies?
- Does line access matter enough when sparse lookup is already fast?
- Do indexes stay small enough to justify themselves?
- Do common queries avoid decoding too much data?

Until those are answered against alternatives, the Phase9 completion proves conformance, not market relevance.

## Why the current plan may be impossible as a product

"Impossible" here does not mean impossible to code. It means the product goal may be internally inconsistent or operationally unreachable.

### 1. Core is stable because it avoids the hard user problem

Core avoids semantic search, mutable updates, ranking, normalized text, and vector behavior. That makes Core implementable. It also means the release candidate does not solve the most visible user problem.

The plan may be impossible if it needs both:

- a small, stable, evidence-only Core
- a product story that feels like search, memory, and retrieval

Those two goals pull in opposite directions.

### 2. Exact evidence and normalized search are in tension

The spec correctly says original text must not be normalized and normalized indexes need mapping metadata before they can prove hits against raw UTF-8 bytes.

That is a hard requirement. For real search, users expect case folding, Unicode normalization, stemming, tokenization, typo tolerance, and language-specific behavior. Each feature increases the distance between query match and original bytes.

The more useful search becomes, the harder exact evidence proof becomes. The more exact evidence proof is preserved, the less friendly search becomes.

### 3. High-performance search may require index sizes users reject

Token indexes, n-gram indexes, skip data, granule tables, boundary windows, and sidecar manifests all add storage. For Japanese substring search, n-gram indexes can become large. For logs, common terms can produce huge posting lists.

The plan includes candidate caps and decoded-byte caps, which are necessary. But caps can create a bad product experience:

```text
The query works only when it is rare enough.
Common queries require narrowing, fallback scans, or higher limits.
```

That may be correct engineering, but it weakens the product promise.

### 4. Document semantics are underspecified for real data

Phase10 adds Document Index and memory profile behavior. The spec defines document ranges, but not the ingestion rules that create them.

In real corpora, document boundaries are messy:

- chat exports
- markdown with embedded frontmatter
- PDFs converted to text
- logs with multiline events
- source code repositories
- issue threads and comments
- partial updates

If document identity is external and optional, QZT cannot be the document system. If QZT tries to become the document system, Core becomes much larger than planned.

### 5. A sidecar-first architecture duplicates search-engine problems

Phase13 introduces memory-mappable sidecars. That quickly leads to the same problems mature search engines already solve:

- index build scheduling
- incremental updates
- shard layout
- compaction
- cache warming
- concurrency
- version compatibility
- partial corruption recovery
- query planning
- multilingual analysis
- benchmark discipline

QZT can implement a subset, but the product then competes with search engines instead of just being a compact evidence format.

## Phase-by-phase counterargument

| Phase | Current plan | Counterargument |
|---:|---|---|
| 0-3 | Format foundation, deterministic CBOR, fixed structures, metadata, index root | These prove a careful binary format, not a user need. They are sunk cost unless a concrete workflow needs QZT specifically. |
| 4-7 | UTF-8 chunking, zstd writer, reader, range and line access | Useful but potentially replaceable by split zstd frames plus an offsets table. The differentiator must be measured, not assumed. |
| 8-9 | dictionaries, resource limits, conformance hardening, release readiness | Good engineering. Still only proves Core correctness. It does not prove adoption, integration, or superiority to existing storage/search stacks. |
| 10 | Dense Line Index, Document Index, memory profile, maintenance command scope | Dense line lookup may be unnecessary if sparse lookup is already fast. Document Index needs ingestion semantics. Maintenance commands expose immutability friction rather than product value. |
| 11 | raw token search MVP | Raw token search is too weak for Japanese, mixed-language corpora, and user-friendly search. It may demonstrate plumbing but not a compelling product. |
| 12 | n-gram index, planner, benchmark reporting | This is where complexity rises sharply. Index size, high-DF terms, boundary matches, and caps may produce a search system that is correct but unpleasant. |
| 13 | sidecar and high-performance search | If the sidecar is the high-value layer, QZT Core must prove why it is the necessary backing store. Otherwise the sidecar design can outgrow the container. |

## Strongest reasons to stop before Phase10

1. Phase9 Core completion is already enough to test the core value hypothesis.
2. Phase10-13 add significant complexity before product pull is proven.
3. The next work item, Dense Line Index, is an optimization, not a market proof.
4. Search work risks turning QZT into a partial search engine.
5. The plan lacks a competitor benchmark gate against simpler architectures.
6. The plan lacks an end-to-end Memory Pager or evidence workflow gate.
7. The plan lacks a kill condition for "QZT is technically correct but unnecessary."

## Product kill criteria

Treat these as reasons to stop or radically narrow QZT:

- A real target workflow cannot name `qzt range`, `qzt line`, or evidence refs as a must-have primitive.
- QZT cannot beat `zstd + offsets manifest` for range and line access by a meaningful margin on large real corpora.
- QZT plus sidecar is not simpler, smaller, or faster than SQLite FTS, Tantivy, Lucene, or another existing search stack for the same job.
- Search indexes exceed acceptable size overhead for target corpora.
- Common queries often hit candidate or decoded-byte caps.
- Evidence refs are not consumed by an end-to-end Memory Pager, audit, or retrieval workflow.
- Recovery requires QZT-specific tooling that operators do not trust for long-term archives.
- Sidecars can provide the same value over raw files or split zstd frames without QZT Core.

## Experiments required to defeat this counterargument

Do not start Phase10 as pure implementation work until at least one product-disproof gate is added.

Recommended gates:

1. Competitive range benchmark:
   Compare QZT against plain `.zst`, split zstd frames plus offsets, and a simple SQLite/blob baseline on 1 GiB, 10 GiB, and 100 GiB text corpora.

2. Evidence workflow demo:
   Build one end-to-end flow where an external memory or audit system stores evidence refs, retrieves exact text through QZT, and verifies those bytes.

3. Search architecture bake-off:
   Before implementing Phase11-13, compare a QZT sidecar design against Tantivy or SQLite FTS over the same source text and evidence-pointer requirements.

4. Dense Line Index kill gate:
   Only implement or keep Dense Line Index if it improves p95 line lookup by a meaningful factor on large files while keeping size overhead within a documented budget.

5. N-gram index kill gate:
   Reject the built-in n-gram path if index size, build time, or common-query fallback behavior is worse than an external search engine.

6. Sidecar necessity gate:
   Prove that `.qzi + .qzt` is materially better than `.qzi + split zstd frames` or `.qzi + raw content-addressed blobs`.

## Recommendation

The adversarial recommendation is:

```text
Pause Phase10-13 implementation.
Keep Phase9 Core as an experimental reference format.
Add a Phase9.5 Product Disproof Gate before further format expansion.
Only continue if QZT wins a real workflow or benchmark against simpler existing stacks.
```

If QZT cannot pass that gate, the project should shrink to one of these:

- a reference experiment for evidence-addressed compressed text
- a library component inside a larger Memory Pager system
- a sidecar index design that does not require a new container format
- a set of conventions for zstd chunk manifests rather than a standalone product

The uncomfortable conclusion is that QZT is technically coherent, but technical coherence is not enough. The current plan still needs proof that a new container format is the product, not merely an implementation detail.
