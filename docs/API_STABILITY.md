# QZT API Stability Policy

Date: 2026-07-19

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

### Writer API consolidation

`WriterBuilder` is the single entry point for optional profiles and indexes.
The crate root retains `pack_bytes` for the common Core-profile case and
`pack_bytes_with_container_id` for deterministic conformance fixtures.

The pre-publication helper aliases were removed before the stable v0.1 crate:

| Removed helper | Migration |
| --- | --- |
| `pack_bytes_with_profile` | `WriterBuilder::new().profile(profile).dense_line_index(enabled).pack(input)` |
| `pack_bytes_with_dense_line_index` | `WriterBuilder::new().container_id(id).dense_line_index(true).pack(input)` |
| `pack_bytes_with_document_index` | `WriterBuilder::new().container_id(id).document_index(index).pack(input)` |
| `pack_bytes_with_memory_profile` | `WriterBuilder::new().container_id(id).profile("memory").document_index(index).pack(input)` |

This is an intentional technical-preview breaking change made while
`publish = false`; it removes overlapping names without changing container
bytes for an equivalent builder configuration.

### Public documentation and lint contract

The default-feature crate denies `missing_docs`, so every crate-root reachable
type, field, variant, function, and method must document its caller-visible
contract before it can compile. Public functions describe failure conditions
with `# Errors` sections where applicable. CI and `make doc` additionally build
all-feature rustdoc with warnings treated as errors.

The curated build no longer suppresses `dead_code` for entire internal modules.
A small number of low-level fixture helpers retain item-scoped allowances with
nearby rationale because they are reachable only through `internal-testing`.
That feature is a conformance-test compatibility surface, not a supported
embedding API.

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
- Issues #22 and #30 are merged, and the dedicated stable release change makes
  the manifest eligible for publication. Actual crates.io publication remains
  an irreversible release-owner-only action and must follow the clean-commit
  gate in [RELEASE.md](RELEASE.md).
