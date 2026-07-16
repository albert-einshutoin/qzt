# QZI v0.1 Sidecar Spec

Date: 2026-07-07

## Scope

This document describes the public on-disk layout of the QZI (`.qzi`) search sidecar shipped in Phase13 of the QZT v0.1 reference implementation.

QZI is a **derived, rebuildable, untrusted** index over a QZT (`.qzt`) Core container. It is not part of the Core container format. For Core container semantics, see [QZT v0.1 Core Spec](QZT_v0.1_Core_Spec.md) Section 30.

```text
.qzt = cold, immutable, verifiable evidence container
.qzi = hot, rebuildable search index bound to one source container
```

Suggested filename: `data.qzt.qzi`.

## Responsibility boundary

| Concern | QZT Core (`.qzt`) | QZI sidecar (`.qzi`) |
| --- | --- | --- |
| Evidence integrity | Authoritative | Derived; must match source |
| Read / export / verify | Always available without sidecar | Optional |
| Search candidate lookup | Not required | Provides granule / term / posting data |
| Hit confirmation | Decodes original bytes | Never sufficient alone |

Readers MUST treat every sidecar byte as untrusted until header, manifest, source binding, and section checks pass.

A sidecar open, parse, or search failure MUST be **fail-closed on the sidecar path only**. Core `open`, `export`, `verify`, and range/line access MUST continue to work when a sidecar is missing, mismatched, or corrupt.

## Physical layout

```text
Offset  Size  Type   Field
0       8     bytes  magic = "QZISIDE1"
8       8     u64le  manifest_size
16      N     cbor   Sidecar Manifest (deterministic CBOR)
16+N    ...   bytes  section payloads (offsets relative to byte 16+N)
```

Rules:

- `manifest_size` is the byte length of the deterministic CBOR manifest.
- Section `offset` values in the manifest are relative to the first payload byte (immediately after the manifest).
- The three payload sections are stored contiguously in build order: `granules`, then `terms`, then `postings`.

## Sidecar manifest

Top-level manifest schema: `qzt.sidecar.v1`.

Required fields (names match the reference encoder in `src/sidecar.rs`):

```yaml
schema: "qzt.sidecar.v1"
source_container_id: bstr16
source_format_version: [0, 1]          # major, minor of the bound QZT container
source_original_checksum:
  algorithm: "blake3"
  value: bstr32
source_qzt_footer_checksum:
  algorithm: "blake3"
  value: bstr32
index_type: "token" | "ngram"
ngram_n: null | u64                    # required when index_type = "ngram"; null for token
complete: bool
high_df_per_million: u32
index_manifest:
  schema: "qzt.search-index.v1"
  kind: string                         # same value as index_type
  posting_granularity: "line"
  index_size_bytes: u64                # total payload size (granules + terms + postings)
  source_size_bytes: u64               # original logical size of the bound container
sections:
  granules: { offset, size, checksum }
  terms:    { offset, size, checksum }
  postings: { offset, size, checksum }
```

Each section reference:

```yaml
offset: u64                            # relative to first payload byte
size: u64
checksum:
  algorithm: "blake3"
  value: bstr32                        # BLAKE3-256 of the section bytes
```

Manifest CBOR MUST use deterministic encoding (canonical map key order and integer widths), matching the Core container CBOR rules.

### Source binding

Before any section is used, readers MUST validate:

1. `schema` is exactly `qzt.sidecar.v1`.
2. `source_format_version` is exactly `[0, 1]`. Any other pair is rejected (`UnsupportedVersion`).
3. `source_container_id` equals the bound container's `container_id`.
4. `source_original_checksum` equals the bound container metadata `original_checksum`.
5. `source_qzt_footer_checksum` equals BLAKE3-256 of the bound container Footer Payload (footer bytes excluding the fixed trailer).

Any mismatch MUST reject the sidecar (`ContainerIdMismatch` or `ContainerCorrupt`). It MUST NOT alter Core read/export/verify behavior.

### Index type rules

- `index_type = "token"`: canonical writers emit `ngram_n = null`. The v0.1
  reference readers accept a non-negative integer for compatibility but ignore
  it on the token path; readers MUST NOT use it to change token semantics.
- `index_type = "ngram"`: canonical writers emit a positive integer. The
  reference reader requires the field to be present; zero cannot form a valid
  n-gram search configuration and search returns an error.

Any other `index_type`, a malformed `ngram_n`, or a missing `ngram_n` on the
n-gram path rejects the sidecar.

## Section payloads

Readers MUST validate each section's `offset`, `size`, and `checksum` against the sidecar file bounds before decoding. Checksum mismatch or out-of-bounds access MUST reject the sidecar.

### `granules` section

Binary layout:

```text
u64le granule_count
repeat granule_count times (56 bytes each):
  u64le granule_id
  u64le logical_offset
  u64le byte_length
  u64le chunk_start
  u64le chunk_end
  u64le first_line      # u64::MAX means absent
  u64le line_count      # u64::MAX means absent
```

Expected section size: `8 + granule_count * 56`. Size mismatch MUST reject the sidecar.

Each granule record maps a posting target to a logical byte range and chunk span in the source container.

### `terms` section

Binary layout:

```text
u64le term_count
repeat term_count times:
  u64le key_len
  key_len bytes key
  16 bytes key_hash
  u64le document_frequency
  u64le granule_frequency
  u64le posting_offset    # relative to start of postings section
  u64le posting_size
  u64le skip_offset
  u64le skip_size
  u64le flags
```

`posting_offset + posting_size` MUST lie within the postings section bounds.

`skip_offset` and `skip_size` are reserved planning metadata in QZI v0.1.
There is no serialized skip-data section and these values have no file-offset
base. Readers MUST NOT seek with them. The reference in-memory reader rebuilds
skip points from decoded posting lists; the file-backed reader ignores these
fields. A future sidecar version must define a new payload contract before
persisted skip data can be consumed.

Term keys are sorted. `key_hash` is a lookup accelerator; exact `key` comparison is still required.

### `postings` section

Concatenated posting lists. Each term's slice `[posting_offset, posting_offset + posting_size)` encodes one sorted granule ID list using `delta-varint-u64-v1`:

```text
first granule_id as unsigned varint
then (granule_id - previous_granule_id) as unsigned varints
```

Posting lists MUST be strictly increasing by `granule_id`. Every referenced `granule_id` MUST be less than `granule_count`.

## Search and verification boundary

A sidecar hit is a **candidate** only. Search MUST:

1. Intersect posting lists for query keys.
2. Resolve candidate granules to chunk spans.
3. Decode the overlapping original bytes from the **source QZT container**.
4. Verify matches against those original bytes (token or n-gram rules).

A sidecar alone MUST NOT be treated as proof of content. Tampered sidecar data that passes section checksums but disagrees with the container will fail original-byte verification or source binding on open.

## Fail-closed summary

The sidecar path MUST reject (non-exhaustive) when:

| Condition | Typical error |
| --- | --- |
| Wrong magic or truncated header | `InvalidHeader` / `UnexpectedEof` |
| Non-deterministic or unknown manifest schema | `ContainerCorrupt` |
| Unsupported `source_format_version` | `UnsupportedVersion` |
| `source_container_id` mismatch | `ContainerIdMismatch` |
| `source_original_checksum` mismatch | `ContainerCorrupt` |
| `source_qzt_footer_checksum` mismatch | `ContainerCorrupt` |
| Section out of bounds | `UnexpectedEof` |
| Section checksum mismatch | `ContainerCorrupt` |
| Granule / term / posting parse failure | `ContainerCorrupt` |
| Integer overflow on bounds arithmetic | `ResourceLimitExceeded` |

None of these failures may prevent Core container operations on the bound `.qzt` file.

## Rebuild and deployment

Sidecars are optional and rebuildable:

```text
qzt sidecar-rebuild input.qzt -o input.qzt.qzi
qzt search input.qzt "query" --sidecar input.qzt.qzi
```

If a sidecar is missing, callers may rebuild a transient in-memory index from the container instead. Core behavior does not depend on the sidecar being present.

## Reference implementation

Phase13 reference code: `src/sidecar.rs`, `tests/phase13_sidecar.rs`.

Conformance vectors for sidecar rejection and Core isolation are exercised in `tests/phase13_sidecar.rs`.

## Related documents

- [QZT v0.1 Core Spec](QZT_v0.1_Core_Spec.md) — Core container format and Search Extension overview (Section 29–30)
- [QZT v0.1 Format Stability](QZT_v0.1_Format_Stability.md) — Core format stability statement
