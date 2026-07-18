# QZT v0.1 Portable Conformance Vector Kit

This directory and the [QZT v0.1 Core specification](https://github.com/albert-einshutoin/qzt/blob/main/docs/QZT_v0.1_Core_Spec.md)
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

Git attributes force LF checkout for the vector and TSV files, so hashes remain
portable when a contributor has automatic line-ending conversion enabled.

## Manifest schema

`manifest.tsv` is UTF-8, tab-separated, and has six columns. The original first
five columns are unchanged; `expect_error` is an appended compatibility column.

| Column | Meaning |
|---|---|
| `name` | File stem of `<name>.qzt.hex`; unique within this kit. |
| `kind` | Encoding of the fixture. Vector set v1 uses `hex`. |
| `expect_open` | `ok` when structural open must succeed; otherwise `err`. |
| `expect_deep_verify` | `ok` or `err` after a successful open; `-` when open must fail. |
| `expect_export_text` | Escaped exact original UTF-8 bytes; `-` only when export must not run. An empty field means zero bytes. |
| `expect_error` | Language-independent snake_case validation category when an expected stage fails; `-` for valid vectors. |

The export grammar recognizes exactly four escapes: `\\` for one backslash,
`\t` for TAB, `\r` for CR, and `\n` for LF. A trailing backslash or any other
escape is invalid. All other Unicode scalar values are encoded directly as
UTF-8, byte-for-byte; runners must not perform Unicode normalization.

Error categories name the failed format validation, not a language-specific
exception class. An implementation maps its native error to the category in
the last column. The Rust names are informative reference mappings only.

| Manifest category | Language-independent failed validation | Rust reference mapping | Stage |
|---|---|---|---|
| `invalid_magic` | Fixed Header bytes 0–7 are not `QZT\0TXT1`. | `InvalidMagic` | open |
| `invalid_footer_trailer` | After the recorded 32-byte truncation, the final 64-byte window cannot decode as a complete fixed Footer Trailer with `QZTTAIL1` magic and required length/version. Native EOF/truncation errors map here for this vector. | `InvalidFooterTrailer` | open |
| `footer_checksum_mismatch` | The Footer Trailer is structurally valid, but BLAKE3 of its referenced Footer Payload differs from the stored trailer checksum. | `FooterChecksumMismatch` | open |
| `non_canonical_cbor` | All enclosing references and checksums are valid, but Metadata map keys are not in deterministic CBOR order. | `NonCanonicalCbor` | open |
| `compressed_chunk_checksum_mismatch` | Structural open succeeds, but BLAKE3 of the stored compressed chunk differs from its Chunk Table entry. | `CompressedChunkChecksumMismatch` | deep verify |

The recorded stage and predicate are part of this kit's contract. A native
reader may expose different exception names, but its adapter must normalize
the failure according to the predicate above.

## Optional index expectations

`extensions.tsv` makes the Dense Line Index and Document Index vectors
observable instead of allowing a reader to ignore their blocks. It records:

- whether each optional block must be present and successfully decoded;
- for the Document Index, the exact `doc_id`, logical range, line range, chunk
  range, algorithm, and document checksum of its single entry.

A full-kit runner must also retrieve `doc-1` by ID, read its recorded logical
range, and verify that the returned 13 bytes equal `document one\n` and match
the recorded BLAKE3 checksum. The Dense Line Index runner must use the decoded
index to resolve the starts of lines 0, 1, and 2 as offsets 0, 5, and 9 within
decoded chunk 0. These are chunk-local offsets, not container logical offsets.

## Conformance rule

Passing the vector-kit Core profile requires all three applicable
`manifest.tsv` checks: structural open, deep verification and error
normalization, and byte-for-byte export. Full QZT vector set v1 conformance
additionally requires every
`extensions.tsv` assertion and lookup above. Implementations that intentionally
omit optional indexes may claim only “QZT vector-kit Core profile passed”; they
must not claim full vector-set conformance. Passing this kit alone does not
prove the broader “Reader Core conformance” defined by the Core specification,
which has additional API and verification requirements. Merely decoding valid
files is not sufficient, and corrupt files must fail at the recorded stage.

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
    expected = strict_unescape(row.expect_export_text, ["\\\\", "\\t", "\\r", "\\n"])
    assert opened.reader.export_all() == utf8_bytes(expected)
```

Then apply every row of `extensions.tsv`, parse the required optional block,
compare every recorded field, and perform the Dense Line and document lookup
checks described above.

## Frozen vector policy

Vector set v1 was published on 2026-07-19 with 14 vectors. In accordance with
the [format stability statement](../../docs/QZT_v0.1_Format_Stability.md), an
existing vector file and its manifest expectations never change. Future sets
may add new rows and files only. A byte change caused by a writer refactor is a
writer regression, not a reason to update an existing vector.

The Rust suite requires the manifest names, committed `.qzt.hex` names, and
frozen BLAKE3 registry to be exactly the same set. It freezes the original 14
manifest rows and both extension rows, stores a raw-file BLAKE3 for every
published vector, and also regenerates each fixture in memory to compare
decoded bytes. Every future published row must be added to the corresponding
frozen row registry in the same change; otherwise tests fail.

Maintainers can generate candidate files with:

```sh
cargo test --all-features --test phase22_vectors -- --ignored regenerate_vectors
```

The ignored test writes with create-new semantics only to
`target/conformance-vectors-candidate/`; it never overwrites the published
`tests/vectors/` files. A second run requires the candidate bytes to be
identical. Compare the candidate directory with this directory before proposing
a new vector. If a candidate name already exists with different bytes, the test
fails and requires the maintainer to remove that ignored candidate explicitly.

Never copy a changed candidate over an already published vector. Additions must
append a manifest row and add the file hash to the frozen registry in the same
reviewed change.
