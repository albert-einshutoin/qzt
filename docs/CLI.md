# qzt CLI Reference (v0.1)

This is the complete command reference and automation stability contract for
the QZT v0.1 technical-preview CLI. Examples on this page were executed against
the fixture described in [Reproducing the examples](#reproducing-the-examples).

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

## Commands

### `qzt help`, `qzt --help`, `qzt --version`

`help`, `-h`, and `--help` print top-level help and exit `0`. `-V` and
`--version` print `qzt <version>` and exit `0`.

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

Stream all original bytes to stdout, or to a newly created/truncated output
file. Opening checks the container structure, and decoding validates each
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
| `--max-candidates <N>` | Candidate granules; default `10000`. |
| `--max-decoded-bytes <N|NKiB|NMiB|NGiB>` | Decode budget; default 256 MiB. Suffixes are case-sensitive. |
| `--max-results <N>` | Result cap; default unlimited (`u64::MAX`). |
| `--format text\|json` | Default text. |

JSON top-level fields are `hits` (array), `metrics` (object), `capped`
(boolean), and `incomplete_reason` (string or null). Each hit has
`logical_offset`, `byte_length`, `chunk_start`, `chunk_end`, and `source`
(`verified_original_bytes`). Metrics contain `query`, `index_kind`,
`posting_granularity`, `index_size_bytes`, `source_size_bytes`,
`index_size_ratio`, `term_lookups`, `posting_bytes_read`,
`candidate_granules`, `candidate_chunks`, `decoded_bytes`,
`physical_decoded_bytes`, `verified_matches`, and `query_time_ms`.

`incomplete_reason` currently uses `query_shorter_than_ngram_n`,
`query_has_no_indexable_tokens`, or
`missing_required_key_in_incomplete_index`. A non-null reason means the
empty/partial result must not be interpreted as a complete negative finding.

### `qzt sidecar-rebuild <FILE> -o <OUTPUT.qzi> [OPTIONS]`

Build a QZI sidecar. Options are `--index token|ngram` (default token),
`--ngram <N>` (default 3), and required `-o, --output`. Search verifies that the
sidecar belongs to the selected container when it opens it.

### `qzt verify <FILE> [--quick|--normal|--deep] [--format text|json]`

Default level is normal. If more than one level flag appears, the last wins.

| Level | Work |
|---|---|
| `quick` | Structural blocks, offsets, schemas, required checksums and limits. |
| `normal` | Quick plus stored compressed-chunk checksums; decoded bytes are zero. |
| `deep` | Normal plus decoding, original-byte checksums, UTF-8/newline/index/document consistency. |

Success JSON contains `ok`, `level`, `checked_chunks`, and `decoded_bytes`.
Failure JSON contains `ok:false`, `level`, and `error`, exits `1`, and is written
to stdout as described in the stability contract.

```json
{"ok":true,"level":"deep","checked_chunks":1,"decoded_bytes":55}
```

### `qzt attest [--level quick|normal|deep] <FILE>`

Default level is deep. The option may precede or follow the file. QZT emits
nothing until verification succeeds, then writes exactly one canonical JSON
line. Top-level fields are `chunk_count`, `container_checksum`, `container_id`,
`final_file_size`, `format`, `line_count`, `original_checksum`, `original_size`,
and `verify`; the nested `verify` object contains `checked_chunks`,
`decoded_bytes`, and `level`. See [Attestation canonical form](#attestation-canonical-form) and the
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
- fields: `chunk_count`, `container_checksum`, `container_id`,
  `final_file_size`, `format`, `line_count`, `original_checksum`,
  `original_size`, and `verify` (`checked_chunks`, `decoded_bytes`, `level`).

Executed fixture output:

```json
{"chunk_count":1,"container_checksum":{"algorithm":"blake3","value":"c0c832eeb45e889673968b846e66abd9a533ccee5c6aa229f521486e195acbd1"},"container_id":"ea4b7a560231e640c9ab0c838cc22a78","final_file_size":2536,"format":"qzt-0.1","line_count":4,"original_checksum":{"algorithm":"blake3","value":"ea4b7a560231e640c9ab0c838cc22a7813bbc864d5a9f8a850df7ca5960dff30"},"original_size":55,"verify":{"checked_chunks":1,"decoded_bytes":55,"level":"deep"}}
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
