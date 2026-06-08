# QZT v0.1 Format Stability Statement

Date: 2026-06-08

QZT v0.1 container bytes are frozen once the technical preview vector set is
published. A reader that supports v0.1 must interpret the fixed header, footer
trailer, deterministic CBOR footer payload, metadata, index root, Chunk Table,
and chunk zstd frames according to `docs/QZT_v0.1_Core_Spec.md`.

## Version Negotiation

- `format_version = 0.1` is accepted by this reader.
- A newer major or minor fixed header version is rejected as
  `UnsupportedVersion`.
- Optional unknown blocks may be ignored only when `required = false`.
- Unknown required blocks are rejected.

## Compatibility Rule

Any byte-layout change to v0.1 structures requires a new `format_version`.
Bug fixes may tighten validation, but must not reinterpret existing valid v0.1
bytes with different semantics.
