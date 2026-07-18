# QZT API Stability Policy

Date: 2026-06-08

QZT v0.1 is a technical preview. The container byte format is compatibility
sensitive; the Rust API is stabilizing through the Product Completeness Track.

## Stable Technical-Preview Surface

Prefer crate-root re-exports:

```rust
use qzt::{
    generate_validation_corpus, Checksum, CorpusKind, QztFileReader, QztReader,
    QztFileWriter, VerifyLevel, WriterBuilder, WriterOptions,
};
```

These are the intended embedding APIs for v0.1:

- `QztReader` and `QztFileReader` for in-memory and positioned reads.
- `QztFileWriter`, `WriterBuilder`, and `WriterOptions` for writing.
- `Checksum`, `VerifyLevel`, `QztError`, and `Result` for verification flows.
- validation corpus helpers used by conformance and benchmark harnesses.

## Compatibility Shims

The historical `pub mod` module paths are available only with the
`internal-testing` feature. This keeps conformance tests and low-level format
fixtures compiling while the default crate surface stays curated through
crate-root re-exports. New code should treat `cbor`, `fixed`, `schema`,
`skeleton`, and `primitives` as internal implementation detail unless a type is
also re-exported at crate root.

## SemVer Policy

- v0.1 container byte-layout changes require a new `format_version`.
- Public crate-root re-export removals require a changelog entry and a minor
  version bump while pre-1.0.
- Internal module changes may happen in patch releases during the technical
  preview.
- The crate is package- and publish-dry-run ready, but `publish = false` remains
  enforced. Actual crates.io publication requires issues #22 and #30 to be
  merged and the release owner to explicitly open the irreversible publish
  gate; see [RELEASE.md](RELEASE.md).
