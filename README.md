# QZT

QZT is a cold evidence container format. This repository contains the Rust reference implementation.

日本語版: [README.ja.md](README.ja.md)

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

## Phase Plan

Implementation proceeds through `tasks/Phase0.md` to `tasks/Phase13.md`.

Progress is tracked in `tasks/status.md`.

## Product Critique

An adversarial counterargument against the current product spec and phase plan is documented in [`docs/QZT_v0.1_Product_Counterargument.md`](docs/QZT_v0.1_Product_Counterargument.md).
