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

- **In-memory reader**: `QztReader` holds the entire container in memory.
  A file-backed seeking reader (`QztFileReader<R: Read + Seek>`) is planned
  before production use.  Large-file support is a post-v0.1 milestone.
- **Transient search index**: `qzt search` without `--sidecar` rebuilds the
  search index on every invocation by reading and decompressing the whole
  container.  For repeated searches, use `qzt sidecar-rebuild` once and then
  `qzt search --sidecar <file.qzi>`.
- **Token search is co-occurrence, not phrase search**: A multi-token query
  `"foo bar"` matches lines that contain both tokens in any order.  Tokens do
  not need to be adjacent.  This is not grep-compatible.
- **Normalized search not implemented**: `SearchIndexSource::NormalizedUtf8`
  (Unicode normalization, case folding, width folding) is not yet implemented.
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
- cargo test --all-targets --all-features
```

## Main CLI

```sh
qzt pack input.txt -o output.qzt
qzt info output.qzt
qzt export output.qzt -o restored.txt
qzt range output.qzt --bytes 0:1024
qzt range output.qzt --lines 1:10
qzt line output.qzt 1
qzt verify output.qzt --deep
qzt sidecar-rebuild output.qzt -o output.qzt.qzi
qzt search output.qzt "error" --sidecar output.qzt.qzi
```

## Documentation

- Core spec summary: [docs/QZT_v0.1_Core_Spec.md](docs/QZT_v0.1_Core_Spec.md)
- Core readiness: [docs/QZT_v0.1_Core_Readiness.md](docs/QZT_v0.1_Core_Readiness.md)
- Release hardening: [docs/QZT_v0.1_Release_Hardening.md](docs/QZT_v0.1_Release_Hardening.md)
- Implementation phases: [tasks/README.md](tasks/README.md)
- Progress: [tasks/status.md](tasks/status.md)

## Phase Plan

Implementation proceeds from [tasks/Phase0.md](tasks/Phase0.md) through
[tasks/Phase13.md](tasks/Phase13.md). Japanese versions are available as
`*.ja.md` files in the same directory.

Progress is tracked in [tasks/status.md](tasks/status.md) and
[tasks/status.ja.md](tasks/status.ja.md).
