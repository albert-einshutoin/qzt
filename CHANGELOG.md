# Changelog

## 0.1.0 - Unreleased

QZT v0.1 is a technical preview reference implementation of the Cold Evidence
Container format for UTF-8 text.

### Release readiness

- Ready for owner-gated crates.io publication after the remaining public API
  and documentation prerequisites are merged. The
  [release checklist](docs/RELEASE.md) records the verified package contents,
  dry-run procedure, irreversible-action boundary, and post-publish checks.

### Added

- Added reproducible GitHub binary distribution with `cargo-dist 0.31.0`,
  tag-only release automation, SHA-256 sidecars, shell and PowerShell
  installers, and four supported macOS/Linux/Windows targets. Both READMEs
  document a version-pinned installer, manual checksum verification, and a
  source-build fallback for the `v0.1.0-pre.2` rehearsal.

### Fixed

- Added Windows positioned file reads and a required Windows release build in
  normal CI after the immutable `v0.1.0-pre.1` rehearsal tag exposed that the
  file-backed partial-decompression and search path compiled only on Unix.

- Added complete English and Japanese CLI references covering every command and
  option, honest profile behavior, range/line conventions, JSON schemas,
  canonical attestation bytes, exit codes, and stdout/stderr compatibility
  rules. Top-level help and both READMEs now link to the v0.1 automation
  stability contract, and every displayed example was executed against the
  documented reproducible fixture.

- Published QZT v0.1 portable conformance vector set v1 with 14 deterministic
  valid and corrupt fixtures covering newline modes, multibyte UTF-8,
  multi-chunk layout, Dense Line and Document indexes, fixed structures,
  checksums, truncation, and non-canonical CBOR. The extended manifest records
  failure stages and language-independent error categories; a separate index
  expectation manifest makes Dense Line and Document Index support observable.
  An implementation guide, LF-stable checkout, isolated candidate generator,
  and per-file BLAKE3 freeze test make the kit usable by independent readers
  and prevent accidental mutation of the public byte contract.

- `qzt attest [--level quick|normal|deep] <file.qzt>` verifies a container
  (`deep` by default) before emitting one deterministic canonical JSON line for
  external signing or RFC 3161 timestamping. The attestation records container
  identity, original and container checksums, sizes, counts, and the exact
  verification coverage. Corrupt inputs exit 1 with empty stdout so a failed
  claim cannot be signed accidentally. The new signing and anchoring guide
  documents minisign, OpenSSL, trust boundaries, and separate proof storage.

- `qzt pack -` reads from stdin instead of a file when the input path is `-`
  (Unix pipeline convention). Example:
  `journalctl --since today | qzt pack - -o today.qzt`
  Stdin input is only supported on the streaming code path
  (`--profile core` without `--dense-line-index on`). Combining `-` with a
  non-streaming profile or `--dense-line-index on` exits with code 2 and prints
  a clear explanation to stderr so users are not surprised by silent OOM on
  large log streams. The `-o <path>` output flag remains required (stdout output
  is not possible because `QztFileWriter` requires seek). The existing atomic
  `.tmp` → rename write and all file-input behaviour are unchanged.

- `qzt search <file.qzt> <query> [--sidecar …] --format json` emits a single
  JSON object to stdout:
  `{"hits":[{"logical_offset":…,"byte_length":…,"chunk_start":…,"chunk_end":…,"source":"verified_original_bytes"},…],"metrics":{"query":"…","index_kind":"…","posting_granularity":"…","index_size_bytes":…,"source_size_bytes":…,"index_size_ratio":…,"term_lookups":…,"posting_bytes_read":…,"candidate_granules":…,"candidate_chunks":…,"decoded_bytes":…,"physical_decoded_bytes":…,"verified_matches":…,"query_time_ms":…},"capped":bool,"incomplete_reason":null|"…"}`.
  Field names match the `SearchReport` / `SearchMetrics` / `SearchHit` Rust
  struct fields directly. `incomplete_reason` is JSON `null` when there is no
  reason; a string value (e.g. `"query_shorter_than_ngram_n"`) when set. The
  `source` field and all string values are passed through `cli_json::escape` so
  queries containing `"` or other special characters never corrupt the JSON.
  The stderr incomplete-result warning is still emitted in JSON mode (stdout
  remains clean). Zero-hit searches emit `"hits":[]`. Unknown `--format` values
  exit with code 2. Default text output is unchanged (existing tests pass
  without modification). `SearchReport`, `SearchHit`, and `SearchMetrics` are
  now exported from the `qzt` crate public API.

- `qzt docs <file.qzt>` lists the Document Index entries in tab-separated columns
  (`doc_id`, `offset`, `bytes`, `first_line`, `lines`, `checksum`); the header is
  the first stdout line. `--format json` emits
  `{"documents":[{"doc_id":"…","logical_offset":…,"byte_length":…,"first_line":…,"line_count":…,"checksum":{"algorithm":"blake3","value":"…"}}]}`.
  `first_line` is converted from its 0-based stored value to 1-based in both text
  and JSON output (aligned with `qzt line` semantics). `doc_id` values containing
  literal tabs or newlines are escaped as `\t` / `\n` in text mode; JSON uses the
  existing `cli_json::escape` helper. A container without a Document Index exits 1
  in both modes (distinct from a valid index with zero entries).
- `qzt doc <file.qzt> <doc_id>` extracts one document by ID with BLAKE3
  verification by default (reads the stored checksum from the Document Index and
  calls `read_document_verified`). `--no-verify` skips the checksum check for a
  faster path. `-o <path>` writes to a file instead of stdout. An unknown `doc_id`
  or a missing Document Index exits 1; a verification failure (corrupt bytes) also
  exits 1. JSON escape and hex helpers from `cli_json` are reused for all output.

- `qzt verify` now prints two report lines after the existing compatibility line:
  `Checked chunks: <n>` and `Decoded bytes: <n>` (decoded bytes is 0 for
  `quick`/`normal` levels, and equals the original size for `deep`).
- `qzt verify --format json` outputs a single-line JSON object to stdout:
  `{"ok":true,"level":"deep","checked_chunks":297,"decoded_bytes":2423996}` on
  success, or `{"ok":false,"level":"deep","error":"..."}` (exit code 1) on
  failure. In JSON mode, all output goes to stdout; stderr is silent so JSON
  consumers need only read stdout.
- Exit codes are now documented in `qzt --help` and in `README.md` /
  `README.ja.md`: `0` = success, `1` = command failed, `2` = usage error.
- `qzt info` now appends three identity lines after the existing output:
  `Container ID` (lowercase hex UUID), `Original checksum` (`blake3:<hex>`),
  and `Newline mode`. Existing lines are unchanged.
- `qzt info --format json` outputs a single JSON object to stdout with all
  container fields including `container_id` and `original_checksum`. Unknown
  `--format` values exit with code 2. JSON is hand-assembled without a serde
  dependency via the new `src/cli_json.rs` helper module (also used by
  subsequent Value Phase 1 commands).
- Deterministic QZT Core writer and reader.
- Chunk Table, sparse line index, optional Dense Line Index, and optional
  Document Index support.
- Raw token and n-gram search MVP plus QZI sidecar technical preview.
- Product Completeness Track: file-backed `QztFileReader`, streaming writer,
  streaming verify/export, competitive benchmarks, resource governance, curated
  public API, verified evidence retrieval, portable conformance vectors, and
  acceptance threshold harness.
- `DocumentEntry::new()` constructor that derives `doc_id_hash` automatically;
  callers no longer need to depend on blake3 directly.
- `#[non_exhaustive]` on `QztError`, `VerifyLevel`, `SearchIndexSource`,
  `SidecarIndexKind`, and `CorpusKind` to allow future variant additions without
  breaking downstream match exhaustiveness.
- `QztError::DocumentNotFound` to distinguish "no document index present" from
  "id not found in index".
- `SearchMetrics::physical_decoded_bytes` reporting the uncompressed bytes
  actually decompressed during hit verification (chunk-level work), alongside
  the logical `decoded_bytes`.
- `SearchReport::incomplete_reason` is now set for queries the index cannot
  answer: `query_shorter_than_ngram_n` (n-gram query with fewer scalars than
  the index `n`) and `query_has_no_indexable_tokens` (token query with no
  ASCII-alphanumeric tokens). The CLI prints the reason and a stderr warning
  instead of returning a confident empty result.
- `QziFileSidecar`: file-backed sidecar reader with lazy lookup. Opening loads
  only the manifest and term dictionary and stream-verifies section checksums;
  posting lists and candidate granule records are fetched per query, so search
  memory scales with the candidate set instead of the sidecar size.
- `RawTokenIndex::build_from_file` / `RawNgramIndex::build_from_file` /
  `build_search_sidecar_from_file`: index construction over `QztFileReader`
  that decodes one chunk at a time (sidecar output is byte-identical to the
  in-memory builder).
- `RawTokenIndex::search_file` / `RawNgramIndex::search_file`: hit
  verification over a file-backed container that decodes only candidate
  chunks.

### Fixed

- Untrusted Chunk Table entries can no longer request an unbounded allocation:
  `ResourceLimits::max_compressed_chunk_size` is enforced during open before a
  chunk is read. File-backed normal verification now streams compressed BLAKE3
  checks through a 64 KiB buffer instead of allocating a full compressed chunk.
- `WriterBuilder::pack` now validates the profile string via `validate_profile`;
  previously any arbitrary string was accepted, producing a container whose
  metadata would be rejected at read time. All pack paths now call
  `validate_profile` unconditionally inside `pack_bytes_internal`.
- The `"memory"` profile requirement that a `DocumentIndex` must be provided is
  now enforced on every pack path, not only on the `WriterBuilder` path.
  Previously `pack_bytes_with_profile` could silently produce a `"memory"`
  container without a Document Index. **Behaviour change**: calls that relied on
  these previously-silent successes now return `QztError::MetadataInvalid`.

### Changed

- Consolidated optional writer profiles and indexes behind `WriterBuilder`.
  Removed the overlapping public helpers `pack_bytes_with_profile`,
  `pack_bytes_with_dense_line_index`, `pack_bytes_with_document_index`, and
  `pack_bytes_with_memory_profile` before the first crates.io publication.
  `pack_bytes` and `pack_bytes_with_container_id` remain for the common Core
  and deterministic-fixture cases; equivalent builder configurations preserve
  container bytes.

- Completed the public API documentation gate: the default-feature crate now
  denies `missing_docs`, all crate-root reachable fields, variants, and methods
  document their caller-visible semantics, and fallible public APIs describe
  error conditions. `make doc` continues to treat every rustdoc warning as an
  error for the all-feature surface.
- Removed the temporary `missing_errors_doc` and `missing_panics_doc` Clippy
  allowances. Module-wide `allow(dead_code)` suppression is gone; the few
  conformance-fixture helpers used only by `internal-testing` now carry narrow,
  reasoned item-level exceptions.

- Migrated the crate and fuzz manifests to Rust 2024 edition.

- `QztError::Display` now emits human-readable messages for all variants instead
  of delegating to `{error:?}` (Debug output). Variant identifiers such as
  `InvalidMagic` no longer appear raw in error text.
- `QztError::NotImplemented` removed; replaced by two purpose-built variants:
  - `QztError::Io(std::io::ErrorKind)` — preserves OS-level I/O error kind
    (file not found, permission denied, write failure, …).
  - `QztError::UnsupportedIndexMode(&'static str)` — signals that the requested
    search index mode is not supported by this implementation.
- `QztFileReader::open_path` and `QziFileSidecar::open_path` now return
  `QztError::Io(kind)` instead of `QztError::ContainerCorrupt` when the
  underlying `File::open` or `File::metadata` call fails.
- `export_to` write failures now return `QztError::Io(kind)` instead of
  `QztError::ContainerCorrupt`.
- `qzt search`, `qzt info`, and `qzt sidecar-rebuild` now run on the
  bounded-memory `QztFileReader` instead of loading the whole container (and
  sidecar) into memory. On a 42 MB / 400K-line corpus with an n-gram sidecar,
  a rare sidecar query dropped from 518 MB to 9.8 MB max RSS (1.33 s to
  0.04 s) and an 80K-hit dense query from 532 MB to 36 MB (1.11 s to 0.17 s).
  Index builds still hold the posting map in memory.
- The search index builder streams chunk-by-chunk and finds chunk spans by
  binary search, removing the O(lines x chunks) granule scan.

- Search hit verification reuses a single-chunk decode cache, so hit-dense
  queries decode each candidate chunk once instead of once per candidate
  granule (measured 16.4 s -> sub-second on a 45 MB corpus with 4,124 hits).
- `qzt export` streams chunks directly to the output file or stdout instead of
  materializing the whole original in memory, matching the documented
  bounded-memory guarantee.
- `make bench-release` now actually builds with `--release`; earlier recorded
  throughput numbers in `tasks/status.md` were debug-build values.
- The quality gate (`make check` and CI) also compiles the default-features
  surface via `cargo check --lib --bins`; internal-testing-only fixture helpers
  use narrow, reasoned item-level `allow(dead_code)` attributes where needed.
- **Public API signatures** (pedantic pass, #9): `pack_bytes_with_document_index`
  and `pack_bytes_with_memory_profile` now take `&DocumentIndex` instead of an
  owned value; `run_release_benchmark_with_corpus` now takes `&[u8]` instead of
  `Vec<u8>`.

### Deferred

- Production-ready performance claims until Phase18 and Phase23 evidence is
  recorded.
