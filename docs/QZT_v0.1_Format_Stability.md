# QZT v0.1 Format Stability Statement

Date: 2026-06-08

QZT v0.1 container bytes are frozen once the technical preview vector set is
published. A reader that supports v0.1 must interpret the fixed header, footer
trailer, deterministic CBOR footer payload, metadata, index root, Chunk Table,
and chunk zstd frames according to `docs/QZT_v0.1_Core_Spec.md`.

## Published Conformance Baseline

Vector set v1 was published on 2026-07-19 with 14 portable fixtures in
`tests/vectors/`. The set covers valid Core features and deterministic corrupt
inputs, with expected structural-open, deep-verification, export, and error
category outcomes recorded in `manifest.tsv`.

The 14 published files and their expectations are immutable. New vectors may
be appended for additional coverage, but an existing vector must not be edited
or reinterpreted. The test suite freezes both regenerated container bytes and a
BLAKE3 digest of every committed `.qzt.hex` file.

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
