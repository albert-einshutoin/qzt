# QZT — Cold Evidence Container for Text

日本語版: [README.ja.md](README.ja.md) · [![CI](https://github.com/albert-einshutoin/qzt/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/albert-einshutoin/qzt/actions/workflows/ci.yml?query=branch%3Amain)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

> Store large text once, prove it later: chunked zstd archives with
> BLAKE3-verified random access, line addressing, and verified search.

## Why QZT

- **Verified evidence** — bind every read and attestation to the original bytes.
- **Random access** — restore byte or line ranges without full decompression.
- **Verified search** — recheck token and n-gram hits against original bytes.

## Install

The currently published CLI is the
[`v0.1.0-pre.2` Release](https://github.com/albert-einshutoin/qzt/releases/tag/v0.1.0-pre.2).
Download its archive and `.sha256` asset for your OS/architecture, verify the
checksum, then extract it. For Apple silicon (use `x86_64-apple-darwin` or
`x86_64-unknown-linux-gnu` for those hosts):

```sh
set -eu
release=v0.1.0-pre.2
target=aarch64-apple-darwin
archive="qzt-${target}.tar.xz"
base="https://github.com/albert-einshutoin/qzt/releases/download/${release}"
curl --proto '=https' --tlsv1.2 -fLO "${base}/${archive}"
curl --proto '=https' --tlsv1.2 -fLO "${base}/${archive}.sha256"
expected="$(awk 'NF { print $1; exit }' "${archive}.sha256")"
if command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "${archive}" | awk '{ print $1 }')"
else
  actual="$(sha256sum "${archive}" | awk '{ print $1 }')"
fi
test "${expected}" = "${actual}"
tar -xJf "${archive}"
QZT_BIN="$(pwd)/qzt-${target}/qzt"
"$QZT_BIN" --version
```

Run the tour below with this absolute `QZT_BIN` path. The published CLI reports
`qzt 0.1.0-pre.2`. The archive checksum is provided by the Release; verify its
authenticity through the Release and repository channel you trust.

<details>
<summary>Windows, installer, and source build options</summary>

<br>

Windows users can download the matching `.zip` and `.zip.sha256` assets and
verify them before extraction:

```powershell
$archive = "qzt-x86_64-pc-windows-msvc.zip"
$expected = (Get-Content "$archive.sha256" | Select-String -Pattern '\S').Line.Split()[0]
$actual = (Get-FileHash -Algorithm SHA256 $archive).Hash
if ($expected -ne $actual) { throw "SHA-256 checksum mismatch" }
```

Alternatively, use `qzt-installer.sh` or `qzt-installer.ps1` from the same
Release. To build **the published version** from source instead of using its
binary:

```sh
cargo install --git https://github.com/albert-einshutoin/qzt --tag v0.1.0-pre.2 --locked qzt
```

`cargo install qzt --version 0.1.0 --locked` becomes an option only after that
version is published on crates.io; it is not the current installation path.

</details>

## 60-second Tour

Use the verified pre.2 binary from above (`QZT_BIN` must be its absolute path).
Run this in a POSIX shell with `mktemp` and `cmp`. It creates a new disposable
directory, independent of existing files:

```sh
set -eu
test -x "$QZT_BIN"
tour_dir="$(mktemp -d)"
cd "$tour_dir"
printf 'alpha\nbeta\nerror gamma\n' > app.log
"$QZT_BIN" pack app.log -o app.qzt
"$QZT_BIN" info app.qzt --format json
"$QZT_BIN" range app.qzt --lines 2:2
"$QZT_BIN" sidecar-rebuild app.qzt -o app.qzt.qzi
"$QZT_BIN" search app.qzt "error" --sidecar app.qzt.qzi --format json
"$QZT_BIN" verify app.qzt --deep --format json
"$QZT_BIN" attest app.qzt > app.attest.json
"$QZT_BIN" export app.qzt -o restored.log
cmp app.log restored.log
```

`info` reports 23 original bytes and 3 lines; the line range prints `beta`
with its newline. Search returns one `error` hit at byte offset 11 with source
`verified_original_bytes`; `verify` reports `ok=true` and `level=deep`.
The pre.2 attestation is deterministic, **versionless** JSON: it has no
`attestation_schema` or later deep-coverage fields. `cmp` succeeds only when
the exported file matches all original bytes. Run the
[release-binary smoke script](docs/guides/examples/smoke-release-tour.sh) with the absolute
binary path for automated JSON, determinism, and byte checks (`jq` required).
See the [measured validation record](docs/guides/tutorial-validation.md).

## Development CLI

The current [CLI reference](docs/CLI.md) and the operational guides below
describe the development implementation at commit
`ad709214f1e8ae18eff6e9f0b633345e40d1617b`, which adds commands,
search budgets, safety fixes, and `qzt-attestation-v1` coverage fields that
pre.2 does not provide. Install that exact source revision with Rust 1.87+:

```sh
cargo install --git https://github.com/albert-einshutoin/qzt \
  --rev ad709214f1e8ae18eff6e9f0b633345e40d1617b --locked qzt
```

Its QZT container format is still `v0.1`; that is separate from the CLI
distribution version. Do not apply the development attestation policy to a
pre.2 JSON document. Feature and limit descriptions below apply to this
development revision unless they explicitly say otherwise.

## Use Cases

- **[Server log preservation](docs/guides/log-preservation.md)** — stream logs
  into daily containers, deep-verify them on a schedule, and anchor
  deterministic attestations separately.
- **[Pipeline artifact fixation](docs/guides/artifact-fixation.md)** — use
  `pack-docs` to bind each input artifact to a named, checksum-verified document
  inside one container.
- **[Incident search operations](docs/guides/search-operations.md)** — search a
  rebuildable QZI sidecar under explicit budgets, then disclose only the
  verified byte or line range needed for an investigation.

## Status & Limitations

QZT v0.1 Core is a release candidate; QZI search and the product remain an
experimental `v0.1 technical preview`, not production-ready software.

```text
- QZT v0.1 Core: release candidate
- Search Extension / QZI sidecar: technical preview
- Product status: experimental reference implementation
```

## What QZT is not

QZT is a `v0.1 technical preview` with deliberately narrow boundaries:

- It is **not a replacement for zstd**. QZT uses independent zstd frames to
  add seeking, integrity metadata, and evidence-oriented indexes.
- It **does not display text without decompression**. A range read decompresses
  only the chunks intersecting that range instead of the whole container.
- It is **not Memory Pager** and does not provide a mutable virtual-memory or
  operating-system paging abstraction.
- It is **not a vector database or FM-index**. QZI provides raw token or n-gram
  candidate lookup and verifies every reported hit against original bytes.

QZI (`.qzi`) is a derived, rebuildable, untrusted search sidecar—not part of
the Core container format. Review its fail-closed boundary and on-disk layout
in the [QZI v0.1 Sidecar Spec](docs/QZI_v0.1_Sidecar_Spec.md) before adoption.
Search rechecks each returned hit against original bytes, including token
boundaries and same-line token AND. `index_complete_declared` reflects an index
declaration; `index_coverage_verified` is currently false, so even an uncapped
zero-hit search does not prove that the source contains no match.

QZT v0.1 is a reference implementation focused on spec coverage and correctness.
Known limitations before production use:

- **Index build memory scales with vocabulary**: every CLI command, including
  `qzt search --sidecar`, now runs on the bounded-memory `QztFileReader`, and
  sidecar search fetches only the queried posting lists and candidate granule
  records (42 MB / 400K-line corpus: rare query 518 MB → 9.8 MB max RSS).
  Building an index (`qzt sidecar-rebuild`, or `qzt search` without
  `--sidecar`) still holds the full posting map in memory — roughly the
  sidecar size expanded — so build sidecars on a machine sized for the corpus.
  Query limits now bound keys, posting processing, verified logical bytes,
  actual chunk decompression, and returned spans; they do not bound the whole
  index build or process RSS. Source lines over 16 MiB are rejected by default.
  See the [search budget table](docs/QZT_v0.1_Memory_Guarantees.md#search-and-index-build-budgets).
- **Transient search index**: `qzt search` without `--sidecar` rebuilds the
  search index on every invocation (chunk-at-a-time decode, but the full index
  stays in memory).  For repeated searches, use `qzt sidecar-rebuild` once and
  then `qzt search --sidecar <file.qzi>`.
- **Token search is co-occurrence, not phrase search**: A multi-token query
  `"foo bar"` matches lines that contain both tokens in any order.  Tokens do
  not need to be adjacent. Tokenization accepts ASCII alphanumerics plus `_`
  and `-`, and lowercases ASCII before indexing. Use the n-gram path for
  substring or CJK-style search. This is not grep-compatible.
- **Normalized search not implemented**: `SearchIndexSource::NormalizedUtf8`
  (Unicode normalization, case folding, width folding) is not yet implemented.
- **Sidecar size**: current writers emit the compact QZI v2 layout. Existing
  QZI v1 sidecars remain readable, but must be rebuilt to receive the v2 space
  reduction. The release gate keeps token and n-gram sidecars at or below 1.7x
  source size on the reproducible 10 MB high-cardinality log corpus; results on
  a different vocabulary or line shape may vary.
- **Benchmark evidence is local, not an SLA**: see the [July 2026 v0.1
  report](docs/benchmarks/2026-07-v0.1.md) for raw-zstd range evidence and
  ripgrep / SQLite FTS5 correctness checks. Tantivy, Lucene, seekable-zstd,
  production logs, and cross-tool search latency remain unmeasured.

### Reproducing the performance numbers

The RSS figures above are local smoke evidence, not an SLA or production
guarantee. Reproduce the release benchmark and profiling run with:

```sh
cargo test --test release_hardening -- --nocapture
make bench-profile
```

For a quicker profile iteration:

```sh
QZT_RELEASE_BENCH_QUERY_REPETITIONS=5 QZT_RELEASE_BENCH_QUERY_WARMUP_REPETITIONS=2 make bench-profile
```

The equivalent convenience target for routine evidence checks is:

```sh
make bench-profile-quick
```

See [the release-hardening guide](docs/QZT_v0.1_Release_Hardening.md) for corpus
details, metric definitions, and additional profiling targets.

### Optional competitive benchmarks

Phase 18 includes an optional competitive benchmark harness. Its measurements
are reproducible local evidence, not an SLA or production performance guarantee.

The portable smoke test does not require external tools:

```sh
cargo test --test phase18_competitive_benchmark -- --nocapture
```

Comparisons with ripgrep and SQLite FTS5 are enabled by `bench-compete`. A
comparator is skipped when `rg` or an FTS5-enabled `sqlite3` is unavailable;
available tools must match the reference byte-scan hit count.

```sh
cargo test --release --all-features --test phase18_competitive_benchmark -- --nocapture
```

See [the competitive benchmark methodology](docs/QZT_v0.1_Competitive_Benchmarks.md)
for details and guidance on when to use QZT.

For the production-scale range-access result, including decoded bytes,
compressed payload reads, and an isolated peak-RSS probe, see the
[1 GiB partial-decompression evidence](docs/benchmarks/2026-07-partial-decompression.md).

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

## Round-trip Smoke Test

The smallest successful path with one text file: pack, inspect, export, and
confirm round-trip equality. QZT is a `v0.1 technical preview`—an experimental
reference implementation, not production-ready software.

From a local checkout, build that checkout's CLI in release mode first. Its
behavior follows the checked-out commit, not the published asset:

```sh
cargo build --release
./target/release/qzt --help
```

The binary remains at `./target/release/qzt` unless you install it on your
`PATH`.

Prepare a plain text file (for example `input.txt`), then:

```sh
./target/release/qzt pack input.txt -o output.qzt
./target/release/qzt info output.qzt
./target/release/qzt export output.qzt -o restored.txt
diff input.txt restored.txt
```

No output from `diff` means the restored bytes match the source.

## CLI Reference

This command map and [the current CLI reference](docs/CLI.md) describe the
[pinned development build](#development-cli). For the published pre.2 binary,
use the [CLI reference at the release tag](https://github.com/albert-einshutoin/qzt/blob/v0.1.0-pre.2/docs/CLI.md)
and the release tour above. QZT format `v0.1` is not a CLI version.

```sh
qzt pack input.txt -o output.qzt
journalctl --since today | qzt pack - -o today.qzt
qzt pack-docs server-a.log server-b.log report.txt -o bundle.qzt
qzt pack-docs server-a.log server-b.log -o logs.qzt --doc-id-prefix logs/ --profile memory
qzt info output.qzt
qzt info output.qzt --format json
qzt attest output.qzt > output.attest.json
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

### Multi-document evidence containers

`qzt pack-docs` concatenates input files in the order given, with no inserted
separator, and records each file as a separately verified document. Document
IDs are the input basenames; use `--doc-id-prefix logs/` to make them
`logs/server-a.log`. Duplicate IDs are rejected as a usage error rather than
silently making `qzt doc` ambiguous.

`pack-docs` reads the complete inputs before packing, so it uses memory
proportional to their total size. Its `memory` profile defaults to a 256 KiB
target and 2 MiB maximum chunk, keeping small document and range reads bounded;
the smaller chunks can reduce compression ratio. Set `--chunk-size` and
`--max-chunk-size` explicitly when a different retrieval/compression trade-off
is appropriate.

Library users can obtain the same generated metadata without calculating chunk
or line fields themselves:

```rust
use qzt::{DocumentSpan, WriterBuilder};

let input = b"first\nsecond\n";
let container = WriterBuilder::new()
    .document_spans(vec![
        DocumentSpan::new("first.txt", 0, 6),
        DocumentSpan::new("second.txt", 6, 7),
    ])
    .pack(input)?;
```

The public Writers reject settings or optional indexes that the default
Readers cannot open and deep-verify. `QztFileWriter` requires an actually empty,
faithful random-access `Read + Write + Seek` sink; it rejects existing bytes
before writing even when positioned at zero or EOF. A zero-length sink with an
advanced seek position is reset to zero. `finish()` flushes and leaves the sink
at EOF, but flush is not filesystem sync. If streaming fails after it begins,
discard its partial output. CLI `pack` and `pack-docs` preserve existing output
through their temporary-file replacement path.

Range semantics: `--bytes A:B` is a half-open byte range `[A, B)`, while
`--lines A:B` is 1-based and inclusive on both ends. `qzt line FILE N` returns
the same raw line bytes as `qzt range FILE --lines N:N` — a convenience wrapper
for a single-line range. On an empty container, `qzt line FILE 1` writes no
stdout, reports that the line is out of range, and exits with exit code **1**.
An n-gram query shorter
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

QZT remains a `v0.1 technical preview`; treat the following as constraints of
the reference implementation rather than production-ready behavior.

### Sidecar is stale or belongs to another container

QZI is bound to one exact QZT container. A stale or mismatched `.qzi` fails
closed on the sidecar path; the source `.qzt` still supports Core read, export,
range access, and verify without that sidecar. Rebuild it from the current
container:

```sh
qzt sidecar-rebuild file.qzt -o file.qzt.qzi
```

### High RSS or OOM during `qzt sidecar-rebuild`

`qzt sidecar-rebuild` decodes the source a chunk at a time, but the v0.1
builder still retains the full term dictionary and posting map while producing
the sidecar. Peak memory therefore grows with corpus vocabulary and posting
volume, and may be much larger than one decoded chunk.

Build the sidecar on a machine sized for the corpus, then reuse it for repeated
queries. `qzt search --sidecar <file.qzi>` uses the file-backed reader and
loads the queried posting lists and candidate granules instead of rebuilding
the complete posting map. This is technical-preview guidance, not a production
memory SLA.

### Search capped at result limit (`capped=true`)

When a search reaches the result cap, the report shows
`capped=true` in the metrics line (text mode) or JSON `"capped": true`. This is
**not** a failure: the command still exits **0** with the hits found up to the
limit. `stop_reason=max_search_results` names the limit. `incomplete_reason`
remains independent: a sidecar declaring `complete=false` retains
`index_complete_declared=false` even when hits are returned or a cap stops the
search. A capped result does not establish whether more matches exist.

Raise the cap with `--max-results <N>` when you need more hits (for example
`qzt search file.qzt needle --max-results 100`).

### `qzt pack -` (stdin) rejects the request

Stdin packing only works with `--profile core` and requires `-o <path>`.
Stdout output, other profiles, and `--dense-line-index on` are unsupported for
stdin input and exit with code **2** as usage errors.

### n-gram query is shorter than index `n`

If a query is shorter than the sidecar's n-gram `n` (default 3), the index
cannot answer it. Search does not report a confident empty result; it prints a
warning and sets `incomplete_reason=query_shorter_than_ngram_n`.

### Token query has no indexable tokens

A token query containing only whitespace, punctuation, or non-ASCII text has
no token keys under the ASCII tokenization contract. Search does not report a
confident empty result; it exits successfully with a warning and sets
`incomplete_reason=query_has_no_indexable_tokens`. This differs from
`query_shorter_than_ngram_n`, which applies only to an n-gram query shorter
than the configured `n`. Use n-gram search for substring or CJK-style input.

### Memory profile requires a Document Index

The `memory` profile requires a Document Index at pack time. `qzt pack` cannot
supply one, so `qzt pack --profile memory` fails with exit code **1** and
points to `WriterBuilder`. Use `qzt pack-docs --profile memory` for file inputs,
or call `WriterBuilder::new().profile("memory").document_index(index)` from
Rust. Choose another profile such as `core` when a Document Index is not
required.

## Documentation

- Reference map: [Documentation index](docs/README.md)
- Vulnerability reporting: [Security Policy](SECURITY.md)
- Contributor workflow: [Contributing Guide](CONTRIBUTING.md)
- Core spec summary: [docs/QZT_v0.1_Core_Spec.md](https://github.com/albert-einshutoin/qzt/blob/main/docs/QZT_v0.1_Core_Spec.md)
- Format stability: [docs/QZT_v0.1_Format_Stability.md](docs/QZT_v0.1_Format_Stability.md)
- Memory guarantees: [docs/QZT_v0.1_Memory_Guarantees.md](docs/QZT_v0.1_Memory_Guarantees.md)
- Competitive benchmark methodology: [docs/QZT_v0.1_Competitive_Benchmarks.md](docs/QZT_v0.1_Competitive_Benchmarks.md)
- Validation corpus: [docs/QZT_v0.1_Validation_Corpus.md](docs/QZT_v0.1_Validation_Corpus.md)
- Public API stability: [docs/API_STABILITY.md](docs/API_STABILITY.md)
- QZI sidecar spec: [docs/QZI_v0.1_Sidecar_Spec.md](docs/QZI_v0.1_Sidecar_Spec.md)
- Attestation signing and anchoring: [docs/guides/attestation.md](docs/guides/attestation.md)
- Core readiness: [docs/QZT_v0.1_Core_Readiness.md](docs/QZT_v0.1_Core_Readiness.md)
- Release hardening: [docs/QZT_v0.1_Release_Hardening.md](docs/QZT_v0.1_Release_Hardening.md)
- Implementation phases: [tasks/README.md](https://github.com/albert-einshutoin/qzt/blob/main/tasks/README.md)
- Progress: [tasks/status.md](https://github.com/albert-einshutoin/qzt/blob/main/tasks/status.md)

## Development

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

Phase docs live in [tasks/](https://github.com/albert-einshutoin/qzt/tree/main/tasks); Japanese versions are available as
`*.ja.md` files in the same directory. Current progress is tracked in
[tasks/status.md](https://github.com/albert-einshutoin/qzt/blob/main/tasks/status.md) and
[tasks/status.ja.md](https://github.com/albert-einshutoin/qzt/blob/main/tasks/status.ja.md).
