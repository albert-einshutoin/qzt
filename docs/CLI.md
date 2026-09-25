# qzt CLI Reference (development CLI; QZT format v0.1)

This page describes the development CLI at commit
`ad709214f1e8ae18eff6e9f0b633345e40d1617b` and its automation contract.
Install that exact revision from the [README development instructions](../README.md#development-cli).
The published `v0.1.0-pre.2` binary predates several commands and JSON fields;
use its [tag-fixed CLI reference](https://github.com/albert-einshutoin/qzt/blob/v0.1.0-pre.2/docs/CLI.md).
`v0.1` identifies the container format, not the CLI distribution. Examples
here use the fixture in [Reproducing the examples](#reproducing-the-examples).

Japanese: [CLI.ja.md](CLI.ja.md)

## Stability contract

### Exit codes

These meanings are frozen for v0.1:

| Code | Meaning |
|---:|---|
| `0` | The command completed successfully. For `verify`, the requested verification passed. |
| `1` | The requested operation failed: unreadable or corrupt input, failed verification, missing document, I/O failure, and similar runtime errors. |
| `2` | Usage error: unknown option, missing argument, or invalid option value. |

### Machine-readable output

- Output explicitly selected with `--format json` is the automation interface.
- Adding a JSON key is backward-compatible. Removing or renaming a key, changing
  its JSON type, or changing its documented meaning is breaking.
- Consumers must ignore unknown keys.
- Object key order and pretty-print whitespace are not stable, except for
  `attest`, whose exact canonical bytes are specified below.
- Integer values are exact. Floating-point formatting, precision, and timing
  values such as `query_time_ms` are not stable.
- Text output is for people. Existing leading lines are preserved where
  practical, but lines may be appended. Do not parse text when JSON exists.

### stdout and stderr

- Successful data goes to stdout, or to the file selected by `-o`.
- Usage errors and ordinary runtime errors go to stderr.
- Warnings and incomplete-search notices go to stderr, including in JSON mode;
  stdout remains valid JSON.
- `verify --format json` is the deliberate exception: verification failure
  writes one `{ "ok": false, ... }` object to stdout, keeps stderr empty, and
  exits `1`.
- `attest` verifies before writing anything. A verification failure leaves
  stdout empty. A stdout I/O failure exits `1` and reports to stderr, but bytes
  already accepted by the output stream cannot be retracted and may be partial.
- No command writes progress output today. Progress output may be added only to
  stderr.

### File output safety

`pack`, `pack-docs`, `export`, `doc`, and `sidecar-rebuild` reject an output that
is the same file as any input, before writing. The check follows input links and
compares filesystem identity (device/inode on Unix; volume/file ID on Windows),
so relative and absolute names, hard links, and case aliases on a
case-insensitive filesystem are covered. An output path that is a symlink,
including a dangling symlink, is rejected even when it points elsewhere.

These commands create a new temporary file in the output directory, preserve
an existing output file's mode and access ACL, complete writing and validation,
flush and sync the temporary file, then replace the output. On Windows, an
existing output with the read-only attribute is rejected before temporary file
creation, without changing its bytes, attribute, or DACL. Writable existing
outputs and new paths retain normal replacement behavior. A failure before
replacement leaves the input and existing output unchanged; a failed new
output has no completed output name. Temporary files are removed on failure;
if removal fails, stderr includes both the original error and the remaining
temporary path. A replacement failure exits `1` and reports that its outcome
must be inspected. A failure **after** replacement during directory durability
confirmation exits `1` and explicitly reports that the output was replaced but
durability is unconfirmed. Do not interpret that result as an unchanged output.

On macOS, access ACLs are copied with `fcopyfile`; on Linux, POSIX access ACLs
are copied through `system.posix_acl_access`; on Windows, the DACL is copied
from the existing output's security descriptor. If that copy fails, the
replacement is abandoned. Ownership, non-access extended attributes, and
filesystem-specific security labels are not part of the preserved metadata.
On macOS and Linux, replacement is a same-directory rename followed by a
`sync_all` of the parent directory. On Windows, replacement uses
`MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` in the same directory. Its
return value confirms the move, but there is no separate portable directory
sync here. Microsoft's explicit flush guarantee for `WRITE_THROUGH` covers
copy-and-delete moves, which this same-volume path does not use; successful
Windows output therefore does not prove crash durability of the directory
entry. These steps do not promise power-loss durability on every filesystem or
network mount. Filesystem changes by another process during the operation
remain outside this CLI contract. stdout commands are streams: bytes already
accepted by stdout cannot be rolled back on later failure.

## Commands

### `qzt help`, `qzt --help`, `qzt version`, `qzt --version`

`help`, `-h`, and `--help` print top-level help and exit `0`. `-V` and
`--version`, as well as the `version` command, print `qzt <version>` and exit
`0`.

### `qzt pack <INPUT|-> -o <OUTPUT> [OPTIONS]`

Pack one UTF-8 byte stream. Options may appear before or after `INPUT`.

| Option | Meaning and default |
|---|---|
| `-o, --output <PATH>` | Required QZT output path. |
| `--profile <PROFILE>` | `minimal`, `core`, `log`, `archive`, or `memory`; default `core`. |
| `--chunk-size <BYTES>` | Target chunk size; default 4 MiB. |
| `--max-chunk-size <BYTES>` | Hard chunk size; default 16 MiB. |
| `--zstd-level <LEVEL>` | zstd level; default `0` (library default). |
| `--checksum blake3` | Only accepted checksum value. |
| `--dict none` | Only accepted dictionary mode; CLI dictionary writing is not implemented. |
| `--dense-line-index on\|off` | Default off, except memory-profile behavior described under Profiles. |
| `-h, --help` | Command help. |

`-` reads stdin only on the streaming path: profile `core`, Dense Line Index
off, and a required file output. stdout cannot be the QZT output because the
writer patches offsets by seeking. File input on this path is also streamed in
64 KiB reads and committed through a unique same-directory temporary file with
an atomic rename. Peak memory includes the chunk buffer plus `O(chunk_count)`
chunk metadata; very small configured chunks increase that metadata. Other
profile/Dense combinations read the complete input into memory. `qzt pack --profile memory`
cannot create the required Document Index and exits `1`; use `pack-docs`.
Pack rejects chunk settings, generated indexes, or document metadata that
exceed default Reader limits. The streaming writer receives an empty
random-access temporary file; its generic API rejects nonempty sinks before
writing and treats a later write/flush failure as partial output. The CLI
discards that temporary output and leaves the existing destination intact.
Flush success alone does not promise power-loss durability; the CLI applies
its separate file-sync and replacement contract.

```sh
journalctl --since today | qzt pack - -o today.qzt
```

### `qzt pack-docs <INPUT>... -o <OUTPUT> [OPTIONS]`

Concatenate files in argument order and create a verified Document Index.
stdin is not supported. Document IDs are `<prefix><basename>` and must be
unique.

Options are the same as `pack`, plus `--doc-id-prefix <PREFIX>`. This command
loads all inputs before packing and uses memory proportional to total input.
For profile `memory`, implicit chunk defaults are 256 KiB target and 2 MiB
maximum; explicit sizes win. Automatic Dense Line Index generation begins at
2048 lines unless forced with `on` or `off`.

```sh
qzt pack-docs alpha.txt beta.txt --doc-id-prefix demo/ -o evidence.qzt
```

### `qzt info <FILE> [--format text|json]`

Print structural metadata. Default format is text. JSON fields are:

| Field | Type | Meaning |
|---|---|---|
| `format` | string | `qzt-0.1`. |
| `container_id` | string | 16-byte ID as 32 lowercase hex characters. |
| `profile` | string | Stored profile declaration. |
| `original_size`, `compressed_size` | integer | Source and final container bytes. |
| `original_checksum` | object | `algorithm` and lowercase-hex `value`. |
| `newline_mode` | string | `none`, `lf`, `crlf`, or `mixed`. |
| `chunk_count`, `line_count` | integer | Stored counts. |
| `zstd_level` | integer | Writer setting. |
| `target_chunk_size`, `max_chunk_size` | integer | Writer settings in bytes. |
| `dense_line_index`, `document_index` | boolean | Optional-block declarations. |
| `document_count` | integer | Zero when no Document Index exists. |

### `qzt export <FILE> [-o <OUTPUT>]`

Stream all original bytes to stdout, or to an atomically replaced output file.
Opening checks the container structure, and decoding validates each
chunk's compressed and uncompressed checksums. It does not validate the
whole-container prefix checksum or aggregate original checksum; run
`qzt verify <FILE> --deep` first when exporting evidence.

### `qzt range <FILE> --bytes A:B|--lines A:B`

- `--bytes A:B` is the zero-based half-open interval `[A, B)`.
- `--lines A:B` is a one-based inclusive interval `[A, B]`.
- `A <= B`; line `A` must be at least 1. Selected original bytes go to stdout.

Executed examples:

```text
$ qzt range evidence.qzt --bytes 0:15
alpha evidence
$ qzt range evidence.qzt --lines 2:3
shared token
beta evidence
```

### `qzt line <FILE> <LINE> [--zero-based]`

Read one original line including its stored newline. The default number is
one-based; `--zero-based` switches to zero-based numbering.

### `qzt docs <FILE> [--format text|json]`

List Document Index entries. Missing Document Index is exit `1`. JSON is
`{"documents":[...]}`; every document contains `doc_id`, `logical_offset`,
`byte_length`, one-based `first_line`, `line_count`, and a checksum object with
`algorithm` and lowercase-hex `value`.

### `qzt doc <FILE> <DOC_ID> [-o <OUTPUT>] [--no-verify]`

Extract one document. By default QZT verifies the Document Index entry checksum
and fails closed. `--no-verify` skips only that document checksum and should be
reserved for diagnosis. Bytes go to stdout unless `-o` is supplied.

### `qzt search <FILE> <QUERY> [OPTIONS]`

Search verified original UTF-8 bytes.

| Option | Meaning and default |
|---|---|
| `--index token\|ngram` | In-memory raw index; default `token`. |
| `--ngram <N>` | N-gram scalar width; default `3`, must be positive. |
| `--sidecar <PATH>` | Use an existing QZI sidecar instead of building an in-memory index. |
| `--max-query-bytes <N|NKiB|NMiB|NGiB>` | UTF-8 query bytes, checked before sidecar-less index build; default 4 KiB. |
| `--max-query-terms <N>` | Distinct normalized token or n-gram keys; default 256. |
| `--max-posting-bytes <N|NKiB|NMiB|NGiB>` | Actual encoded posting bytes per query; default 128 MiB. |
| `--max-posting-ids <N>` | Selected posting IDs per query; default 10,000,000. |
| `--max-posting-work <N>` | ID copies, comparisons, and intersection output pushes; default 20,000,000. |
| `--max-candidates <N>` | Candidate granules; default `10000`. |
| `--max-decoded-bytes <N|NKiB|NMiB|NGiB>` | Logical candidate and adjacent token-boundary bytes read; default 256 MiB. |
| `--max-physical-decoded-bytes <N|NKiB|NMiB|NGiB>` | Full chunk decompression bytes; default 256 MiB. |
| `--max-physical-decoded-chunks <N>` | Chunk decompression calls; default 10,000. |
| `--max-line-bytes <N|NKiB|NMiB|NGiB>` | Source line bytes for index build without `--sidecar`; default 16 MiB. |
| `--max-results <N>` | Result cap; default 10,000. |
| `--format text\|json` | Default text. |

Byte suffixes are case-sensitive. Zero allows no work in the named unit.
Query/posting/index-build overruns exit `1` with no successful report;
candidate, logical/physical decode, and result limits return a capped report
with only verified hits. The [budget table](QZT_v0.1_Memory_Guarantees.md#search-and-index-build-budgets)
defines the exact accounting and check points.

QZI search checks fetched granule ranges against the bound QZT Chunk Table
before counting candidate chunks or returning hit coordinates. File-backed
search may stop at a candidate cap before fetching granules; then
`candidate_chunks` is `0` and unread granules have not been validated.

JSON top-level fields are `hits` (array), `metrics` (object), `capped`
(boolean), `stop_reason` (string or null), `index_complete_declared`
(boolean), `index_coverage_verified` (boolean), and `incomplete_reason`
(string or null). Each hit has
`logical_offset`, `byte_length`, `chunk_start`, `chunk_end`, and `source`
(`verified_original_bytes`). Metrics contain `query`, `index_kind`,
`posting_granularity`, `index_size_bytes`, `source_size_bytes`,
`index_size_ratio`, `term_lookups`, `posting_bytes_read`,
`candidate_granules`, `candidate_chunks`, `decoded_bytes`,
`physical_decoded_bytes`, `physical_decoded_chunks`, `verified_matches`, and `query_time_ms`.

Text metrics include both index fields and the same `stop_reason` (`none` when
absent). `index_complete_declared` reflects the in-memory index flag or QZI
manifest; it is not proof of source coverage. `index_coverage_verified` is
currently `false`, including for `complete=true` and ordinary zero-hit
results. Section checksums and source binding do not prove that all matches
have postings. `source=verified_original_bytes` applies only to returned hits:
token hits require all query tokens on one original line and complete token
boundaries, including a byte outside the granule when needed. These boundary
reads count toward logical and physical decode budgets.

`capped=true`
always has a named stop reason; `capped=false` has none. A capped zero-hit result
is distinct from an ordinary zero-hit result. `posting_bytes_read` is a planner
estimate for transient n-gram search, not the actual posting-byte budget meter.

`incomplete_reason` currently uses `query_shorter_than_ngram_n`,
`query_has_no_indexable_tokens`, or
`missing_required_key_in_incomplete_index`. A non-null reason means the
empty/partial result must not be interpreted as a complete negative finding.
Even with a null reason and no cap, an unverified index cannot establish
absence. A result cap may stop at the limit without proving more hits exist.

### `qzt inspect-sidecar <FILE.qzt> --sidecar <FILE.qzi> [--format text|json]`

Open the QZT through `QztFileReader`, validate every QZI section checksum, and
verify the sidecar's source binding before printing metadata. The default text
output and JSON output contain `index_type`, `ngram_n`, `complete`,
`high_df_per_million`, `source_size_bytes`, `index_size_bytes`,
`granule_count`, `term_count`, and `postings_size_bytes`. A corrupt or
mismatched sidecar exits `1` without printing a successful summary. Inspection
does not upgrade the QZT from quick structural validation; use
`qzt verify <FILE.qzt> --deep` for complete Core verification.
`complete` is the sidecar manifest's declaration, not verified search
coverage; successful inspection does not prove that every match has a posting.

### `qzt sidecar-rebuild <FILE> -o <OUTPUT.qzi> [OPTIONS]`

Build a QZI sidecar. Options are `--index token|ngram` (default token),
`--ngram <N>` (default 3), `--max-line-bytes <N|NKiB|NMiB|NGiB>`
(default 16 MiB, including LF and optional CR), and required `-o, --output`.
An oversized line fails before key generation. Search verifies that the
sidecar belongs to the selected container when it opens it.

### `qzt verify <FILE> [--quick|--normal|--deep] [--format text|json]`

Default level is normal. If more than one level flag appears, the last wins.

| Level | Work |
|---|---|
| `quick` | Structural blocks, offsets, schemas, required checksums and limits. |
| `normal` | Quick plus every stored compressed-chunk checksum and the optional container prefix checksum; decoded bytes are zero. |
| `deep` | Normal plus decoding, original-byte checksums, UTF-8/newline/index/document consistency. |

Success text keeps its first three lines and appends the compressed checksum
chunk count, decoded chunk count, original checksum state, and statuses of the
optional prefix checksum, Dense Line Index, and Document Index. Success JSON
keeps `ok`, `level`, `checked_chunks`, and `decoded_bytes` and adds the same
fields: `compressed_checksum_chunks`, `decoded_chunks`,
`original_checksum_verified`, `container_checksum_status`,
`dense_line_index_status`, and `document_index_status`.
`checked_chunks` always counts Chunk Table entries validated at open; it does
not claim payload verification. Chunk counters count distinct chunks, not
internal hash invocations. All values describe this requested pass, even if
the same Reader previously performed a deeper pass.

The prefix status is `absent`, `present_unchecked`, or `verified`. Index statuses
are `absent`, `stored_block_verified`, or `source_checked`; the stored state
covers saved block checksum, schema, container binding, and the block
descriptor's physical storage range. It does not validate Document Index
logical ranges or chunk spans. Deep checks DLI offsets against decoded bytes.
Deep Document Index checks document hashes, logical range bounds, and chunk
spans, but does not prove exact line coordinates.
Unknown optional blocks are excluded. The optional prefix checksum covers
bytes before the Footer Payload, not the entire file.
Failure JSON contains `ok:false`, `level`, and `error`, exits `1`, and is written
to stdout as described in the stability contract.

```json
{"ok":true,"level":"deep","checked_chunks":1,"compressed_checksum_chunks":1,"decoded_chunks":1,"decoded_bytes":11,"original_checksum_verified":true,"container_checksum_status":"verified","dense_line_index_status":"absent","document_index_status":"absent"}
```

### `qzt attest [--level quick|normal|deep] <FILE>`

Default level is deep. The option may precede or follow the file. QZT emits
nothing until verification succeeds, then writes exactly one canonical JSON
line. The top-level `attestation_schema` is `qzt-attestation-v1`; `format` stays
`qzt-0.1` because it identifies QZT bytes. Other top-level fields are
`chunk_count`, `container_checksum`, `container_id`,
`final_file_size`, `format`, `line_count`, `original_checksum`, `original_size`,
and `verify`; the nested `verify` object has the same report fields as
`verify --format json`, except `ok`: `checked_chunks`,
`compressed_checksum_chunks`, `container_checksum_status`, `decoded_bytes`,
`decoded_chunks`, `dense_line_index_status`, `document_index_status`, `level`,
and `original_checksum_verified`. See [Attestation canonical form](#attestation-canonical-form) and the
[signing guide](guides/attestation.md).

## Profiles

| Profile | v0.1 behavior |
|---|---|
| `minimal` | Metadata declares `minimal`; CLI pack uses the complete-input path. No optional index by default. |
| `core` | Default. With Dense off, single-input `pack` streams payload data; memory is the chunk buffer plus `O(chunk_count)` metadata, not a constant-memory SLA. |
| `log` | Metadata declares `log`; physical layout is otherwise the same as core for identical options. Complete-input CLI path. |
| `archive` | Metadata declares `archive`; physical layout is otherwise the same as core for identical options. Complete-input CLI path. |
| `memory` | Requires a Document Index, so use `pack-docs`. Uses retrieval-oriented chunk defaults there and automatic Dense generation for at least 2048 lines. |

In v0.1, `minimal`, `log`, and `archive` are honest purpose declarations, not
separate compression algorithms. Chunk settings and optional indexes—not the
profile name alone—create most physical differences.

## JSON examples

The fixture produces this `info` identity (whitespace is not contractual):

```json
{"format":"qzt-0.1","container_id":"ea4b7a560231e640c9ab0c838cc22a78","profile":"core","original_size":55,"compressed_size":2536,"original_checksum":{"algorithm":"blake3","value":"ea4b7a560231e640c9ab0c838cc22a7813bbc864d5a9f8a850df7ca5960dff30"},"newline_mode":"lf","chunk_count":1,"line_count":4,"zstd_level":0,"target_chunk_size":4194304,"max_chunk_size":16777216,"dense_line_index":false,"document_index":true,"document_count":2}
```

`docs --format json` returns two entries for `demo/alpha.txt` at offset 0,
length 28, first line 1 and `demo/beta.txt` at offset 28, length 27, first line
3. `search shared --format json` returns verified hits at logical offsets 15
and 42. Timing and floating-point formatting are intentionally omitted here
because they are not stable.

## Attestation canonical form

Unlike other JSON output, attest bytes are stable and signable:

- one object on one line, no insignificant spaces;
- top-level and nested keys in lexicographic order;
- lowercase hexadecimal, JSON integer counts/sizes, and `null` only for a
  legacy missing `container_checksum`;
- no path, host, clock, locale, or other environment-dependent value;
- exactly one trailing LF;
- fields: `attestation_schema`, `chunk_count`, `container_checksum`, `container_id`,
  `final_file_size`, `format`, `line_count`, `original_checksum`,
  `original_size`, and `verify` (the coverage fields described above).

Versionless canonical output from before #292 is legacy v0. Keep its original
bytes and signature together. Recreate those bytes with the pinned old CLI
commit noted in the [signing guide](guides/attestation.md); the current CLI
emits v1 and requires a new signature or timestamp. Different attestation
bytes after a CLI upgrade alone do not indicate changed QZT content.

Executed `tests/vectors/valid_c1.qzt.hex` fixture output:

```json
{"attestation_schema":"qzt-attestation-v1","chunk_count":1,"container_checksum":{"algorithm":"blake3","value":"d05f9357b3182e0e164b508b6cdfd1a2f421559df6886ee2701b330cd5b3a32d"},"container_id":"9885af894b1ee70d8c2cda08e9c68b81","final_file_size":1854,"format":"qzt-0.1","line_count":2,"original_checksum":{"algorithm":"blake3","value":"9885af894b1ee70d8c2cda08e9c68b813aec801465b87a0c16d355d7413b32b7"},"original_size":11,"verify":{"checked_chunks":1,"compressed_checksum_chunks":1,"container_checksum_status":"verified","decoded_bytes":11,"decoded_chunks":1,"dense_line_index_status":"absent","document_index_status":"absent","level":"deep","original_checksum_verified":true}}
```

## Limitations

- CLI dictionary writing is not implemented; only `--dict none` is accepted.
- Normalized/tokenized Unicode search is not provided. Raw token search is
  ASCII-alphanumeric token based; n-gram search uses raw UTF-8/scalars.
- stdout container writing is unsupported because QZT output requires seeking.
- `pack-docs` is not streaming and basenames must produce unique UTF-8 IDs.
- This is a technical preview. The explicit v0.1 contracts above are stable;
  unspecified presentation and performance details are not.

## Reproducing the examples

Run this block from the repository root. All displayed output was executed with
the repository binary and these exact inputs (LF endings):

```sh
set -eu
cargo build --all-features --bin qzt
QZT_BIN="$(pwd)/target/debug/qzt"
QZT_EXAMPLE_DIR="$(mktemp -d)"
trap 'rm -rf -- "$QZT_EXAMPLE_DIR"' EXIT
cd "$QZT_EXAMPLE_DIR"
printf 'alpha evidence\nshared token\n' > alpha.txt
printf 'beta evidence\nshared token\n' > beta.txt
"$QZT_BIN" pack-docs alpha.txt beta.txt --doc-id-prefix demo/ -o evidence.qzt
"$QZT_BIN" info evidence.qzt --format json
"$QZT_BIN" verify evidence.qzt --deep --format json
"$QZT_BIN" range evidence.qzt --bytes 0:15
"$QZT_BIN" range evidence.qzt --lines 2:3
"$QZT_BIN" line evidence.qzt 1
"$QZT_BIN" docs evidence.qzt --format json
"$QZT_BIN" doc evidence.qzt demo/beta.txt
"$QZT_BIN" search evidence.qzt shared --format json
"$QZT_BIN" sidecar-rebuild evidence.qzt --index token -o evidence.qzi
"$QZT_BIN" search evidence.qzt shared --sidecar evidence.qzi --format json
"$QZT_BIN" attest evidence.qzt --level deep
"$QZT_BIN" export evidence.qzt -o exported.txt
```

The final exported file was compared byte-for-byte with the concatenated two
inputs. Search `query_time_ms` differed between runs, as permitted by the
stability contract.
