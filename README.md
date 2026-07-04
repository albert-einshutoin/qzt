# QZT

日本語版: [README.ja.md](README.ja.md)

QZT is a binary format for storing large text as a cold evidence container.
This repository contains the Rust reference implementation.

QZT does not try to build a better compression algorithm than zstd. It splits
source text into independent zstd chunks and combines them with verifiable
metadata, a Chunk Table, a Footer, and a search sidecar so callers can restore
only the required range and return to the original evidence position.

## Current Status

```text
- QZT v0.1 Core: release candidate
- Search Extension / QZI sidecar: technical preview
- Product status: experimental reference implementation
```

When publishing QZT externally, it should be positioned as a
`v0.1 technical preview`, not as production-ready software.

## v0.1 Technical Preview — Limitations

QZT v0.1 is a reference implementation focused on spec coverage and correctness.
Known limitations before production use:

- **Index build memory scales with vocabulary**: every CLI command, including
  `qzt search --sidecar`, now runs on the bounded-memory `QztFileReader`, and
  sidecar search fetches only the queried posting lists and candidate granule
  records (42 MB / 400K-line corpus: rare query 518 MB → 9.8 MB max RSS).
  Building an index (`qzt sidecar-rebuild`, or `qzt search` without
  `--sidecar`) still holds the full posting map in memory — roughly the
  sidecar size expanded — so build sidecars on a machine sized for the corpus.
- **Transient search index**: `qzt search` without `--sidecar` rebuilds the
  search index on every invocation (chunk-at-a-time decode, but the full index
  stays in memory).  For repeated searches, use `qzt sidecar-rebuild` once and
  then `qzt search --sidecar <file.qzi>`.
- **Token search is co-occurrence, not phrase search**: A multi-token query
  `"foo bar"` matches lines that contain both tokens in any order.  Tokens do
  not need to be adjacent.  This is not grep-compatible.
- **Normalized search not implemented**: `SearchIndexSource::NormalizedUtf8`
  (Unicode normalization, case folding, width folding) is not yet implemented.
- **Sidecar size**: the QZI token/n-gram sidecars are uncompressed MVP
  structures. On a realistic 45 MB log corpus the token sidecar measured about
  2.1x the original text; budget sidecar storage accordingly.
- **No production benchmark**: No comparison against SQLite FTS, Tantivy,
  Lucene, or seekable-zstd has been conducted for v0.1.

## Local Quality Gate

```sh
make check
```

The gate runs:

```text
- cargo fmt --all -- --check
- cargo clippy --all-targets --all-features -- -D warnings
- cargo check --lib --bins
- cargo test --all-targets --all-features
```

## Main CLI

```sh
qzt pack input.txt -o output.qzt
journalctl --since today | qzt pack - -o today.qzt
qzt info output.qzt
qzt info output.qzt --format json
qzt export output.qzt -o restored.txt
qzt range output.qzt --bytes 0:1024
qzt range output.qzt --lines 1:10
qzt line output.qzt 1
qzt docs output.qzt
qzt docs output.qzt --format json
qzt doc output.qzt report-2026-06
qzt doc output.qzt report-2026-06 -o out.txt
qzt doc output.qzt report-2026-06 --no-verify
qzt verify output.qzt --deep
qzt sidecar-rebuild output.qzt -o output.qzt.qzi
qzt search output.qzt "error" --sidecar output.qzt.qzi
qzt search output.qzt "error" --sidecar output.qzt.qzi --format json
```

Range semantics: `--bytes A:B` is a half-open byte range `[A, B)`, while
`--lines A:B` is 1-based and inclusive on both ends. An n-gram query shorter
than the index `n` (default 3) cannot be answered by the index; instead of a
confident empty result the CLI reports
`incomplete_reason=query_shorter_than_ngram_n` and prints a warning.

## Exit Codes

```text
Exit codes:
  0  success (verify: container is valid)
  1  command failed (verify: container is corrupt or unreadable)
  2  usage error (unknown option / missing argument)
```

## Troubleshooting

Common CLI failure modes. QZT remains a `v0.1 technical preview`; treat these
as expected constraints of the reference implementation, not production bugs.

### `qzt pack -` (stdin) rejects the request

Stdin packing only works on the streaming core path: `--profile core` without
Dense Line Index (`--dense-line-index on` is not supported). `-o <path>` is
always required; stdout output is not supported. Other profiles, dense line
index mode, or a missing `-o` exit with code **2** and print a usage-style
error.

### n-gram query shorter than index `n`

If a query is shorter than the sidecar's n-gram `n` (default 3), the index
cannot answer it. The CLI does **not** return a confident empty result; search
reports `incomplete_reason=query_shorter_than_ngram_n` and prints a warning.

### memory profile requires a Document Index

The memory profile (`"memory"`) requires a Document Index at pack time. The `qzt pack`
CLI does not accept a Document Index, so `qzt pack --profile memory` is
rejected (`MetadataInvalid`, exit **1**). Use the writer API
(`pack_bytes_with_memory_profile`) with a `DocumentIndex`, or pack with another
profile (for example `core`).

## Documentation

- Core spec summary: [docs/QZT_v0.1_Core_Spec.md](docs/QZT_v0.1_Core_Spec.md)
- Core readiness: [docs/QZT_v0.1_Core_Readiness.md](docs/QZT_v0.1_Core_Readiness.md)
- Release hardening: [docs/QZT_v0.1_Release_Hardening.md](docs/QZT_v0.1_Release_Hardening.md)
- Implementation phases: [tasks/README.md](tasks/README.md)
- Progress: [tasks/status.md](tasks/status.md)

## Phase Plan

Implementation proceeded in two tracks, all phases complete:

- **v0.1 Core (Phase 0–13)**: deterministic CBOR, fixed structures, UTF-8
  chunker, no-dictionary zstd writer, reader open/info/export, verify levels,
  sparse/dense line index, document index, dictionaries, resource limits, and
  the transient search extension with QZI sidecar.
- **Product Completeness (Phase 14–23)**: open-source hygiene, file-backed
  seeking reader (`QztFileReader`), streaming verify/export/writer, competitive
  benchmarks, resource governance, a curated public API, verified evidence
  retrieval, and portable conformance vectors with a frozen format-stability
  statement.

Phase docs live in [tasks/](tasks/); Japanese versions are available as
`*.ja.md` files in the same directory. Current progress is tracked in
[tasks/status.md](tasks/status.md) and [tasks/status.ja.md](tasks/status.ja.md).
