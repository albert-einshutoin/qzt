# QZT v0.1 Portable Conformance Vector Kit

This directory and the [QZT v0.1 Core specification](../../docs/QZT_v0.1_Core_Spec.md)
are sufficient to test an independent QZT reader. The kit covers empty and
multi-chunk containers, newline modes, multibyte UTF-8, optional Dense Line and
Document indexes, and corruption detected during structural open or deep
verification.

## Hex file format

Each `<name>.qzt.hex` file contains one lowercase hexadecimal representation of
the complete QZT byte stream:

- exactly two hexadecimal characters per byte;
- no `0x` prefix, spaces, separators, or internal line breaks;
- one final LF after the hexadecimal line;
- decode the hexadecimal text before passing bytes to a QZT reader.

## Manifest schema

`manifest.tsv` is UTF-8, tab-separated, and has six columns. The original first
five columns are unchanged; `expect_error` is an appended compatibility column.

| Column | Meaning |
|---|---|
| `name` | File stem of `<name>.qzt.hex`; unique within this kit. |
| `kind` | Encoding of the fixture. Vector set v1 uses `hex`. |
| `expect_open` | `ok` when structural open must succeed; otherwise `err`. |
| `expect_deep_verify` | `ok` or `err` after a successful open; `-` when open must fail. |
| `expect_export_text` | Exact original UTF-8 bytes after replacing `\r` and `\n`; `-` when export is not expected. An empty field means zero bytes. |
| `expect_error` | Stable snake_case category when an expected stage fails; `-` for valid vectors. |

Error categories correspond to `QztError` variants as follows:

| Manifest category | QztError variant | Detection stage |
|---|---|---|
| `invalid_magic` | `InvalidMagic` | open |
| `invalid_footer_trailer` | `InvalidFooterTrailer` | open |
| `footer_checksum_mismatch` | `FooterChecksumMismatch` | open |
| `non_canonical_cbor` | `NonCanonicalCbor` | open |
| `compressed_chunk_checksum_mismatch` | `CompressedChunkChecksumMismatch` | deep verify |

## Conformance rule

A reader conforms to this vector set only when all three applicable checks
match the manifest: structural open result, deep verification result and error
category, and byte-for-byte export of valid content. Merely decoding the valid
files is not sufficient; corrupt files must fail at the recorded stage.

Language-independent runner pseudocode:

```text
manifest = read_tsv("manifest.tsv")
for row in manifest.rows:
  bytes = hex_decode(read_text(row.name + ".qzt.hex"))
  opened = reader.open(bytes)
  assert result(opened) == row.expect_open
  if opened failed:
    assert category(opened.error) == row.expect_error
    continue
  verified = opened.reader.verify(level = deep)
  assert result(verified) == row.expect_deep_verify
  if verified failed:
    assert category(verified.error) == row.expect_error
    continue
  if row.expect_export_text != "-":
    expected = unescape_cr_lf(row.expect_export_text)
    assert opened.reader.export_all() == utf8_bytes(expected)
```

## Frozen vector policy

Vector set v1 was published on 2026-07-19 with 14 vectors. In accordance with
the [format stability statement](../../docs/QZT_v0.1_Format_Stability.md), an
existing vector file and its manifest expectations never change. Future sets
may add new rows and files only. A byte change caused by a writer refactor is a
writer regression, not a reason to update an existing vector.

The Rust suite stores a BLAKE3 hash for every published `.qzt.hex` file and
fails if file bytes change. It also regenerates each fixture in memory and
compares the decoded bytes. Maintainers can deliberately regenerate candidate
files with:

```sh
cargo test --all-features --test phase22_vectors -- --ignored regenerate_vectors
```

Run the command twice and require an empty `git diff` before proposing any new
vector. Never regenerate an already published vector as part of routine test or
writer maintenance.
