# QZT v0.1 Core Specification

Status: Draft Complete  
Spec version: 0.1.0  
Date: 2026-06-07
File extension: `.qzt`  
Name: **QZT: Queryable Zstd Text Container**  
Tagline: **A seekable, verifiable, evidence-native text container built on independent zstd chunks.**

---

## 0. Abstract

QZT is a binary container format for large text data.

QZT stores original text as independent zstd-compressed chunks and adds enough structure to let software:

- verify the container without exporting everything,
- restore the original input exactly,
- read only the required byte range,
- read by logical line number,
- trace summaries or memory records back to exact source text,
- optionally use document/search side indexes without making them part of the core format.

QZT is not a replacement for zstd.  
QZT uses independent Zstandard frames as its compression engine and defines a text-oriented container layer above them.

The core product position is:

```text
QZT is the Cold Evidence Container for large text and AI memory systems.
```

In other words, QZT is not the LLM memory manager.  
It is the immutable, seekable, verifiable source-evidence layer that memory systems can reference.

---

## 1. Scope

### 1.1 QZT v0.1 Core includes

A conforming QZT v0.1 Core container MUST support:

```text
- exact export:
    export(pack(input)) == input

- independent zstd chunks

- fixed 128-byte header

- canonical CBOR metadata block

- chunk table

- sparse line index through chunk table:
    first_line + line_count per chunk

- fixed 64-byte footer trailer

- footer payload

- index root block directory

- byte range read

- line read

- quick / normal / deep verification

- UTF-8 chunk boundary safety

- compressed and uncompressed chunk checksums

- immutable container semantics after finish()
```

### 1.2 QZT v0.1 Core excludes

The following are NOT part of QZT v0.1 Core:

```text
- semantic search
- vector embeddings
- vector database behavior
- LLM memory ranking
- summarization
- mutable database updates
- arbitrary binary archive semantics
- ZIP compatibility
- .zst stream compatibility
- mandatory token/ngram search
- mandatory document index
- mandatory quantum-inspired optimizer
```

These may be defined as optional extension specs.

### 1.3 Core conformance and profiles

Profiles tune default writer behavior and optional indexes.
Profiles do not weaken QZT v0.1 Core conformance.

A container that claims QZT v0.1 Core conformance MUST contain all required Core structures, even if its metadata profile is `"minimal"`.

A tool that omits line access, sparse line index fields, verification levels, or required dictionary reading behavior MUST NOT claim QZT v0.1 Reader Core or Writer Core conformance.

The `"memory"` profile is an extension profile. A memory-profile container MAY still be a valid Core container if all required Core structures are present and all extension blocks are optional.

---

## 2. Product boundary

QZT MUST be positioned as:

```text
seekable + verifiable + evidence-addressable text container
```

QZT MUST NOT claim:

```text
- better compression algorithm than zstd
- decompression-free arbitrary text display
- semantic similarity search without external vector/semantic layer
- replacement for FM-index
- replacement for vector DB
- replacement for Memory Pager
```

Correct wording:

```text
QZT enables access to compressed text without full decompression.
QZT narrows access to the relevant compressed chunks and partially decodes only what is needed.
```

Incorrect wording:

```text
QZT reads arbitrary compressed text with zero decompression.
```

---

## 3. Relationship to Memory Pager

Memory Pager and QZT are separate layers.

```text
Memory Pager:
  - extracts atomic memory
  - builds memory hierarchy
  - performs semantic search
  - manages current state
  - ranks memory
  - assembles LLM context

QZT:
  - stores original text losslessly
  - compresses original evidence
  - partially restores byte ranges / lines / documents
  - verifies container and evidence integrity
  - provides stable evidence pointers
```

Dependency direction:

```text
Memory Pager uses QZT.
QZT does not depend on Memory Pager.
```

A memory system may store references like:

```json
{
  "memory_id": "mem_001",
  "summary": "User is designing QZT as a cold evidence container.",
  "evidence_refs": [
    {
      "container_id": "b6a7c34f91a849c8936d91f4d5d06f20",
      "container": "workspace.qzt",
      "doc_id": "conversation_2026_06_06",
      "byte_range": [1048576, 1059321],
      "line_range": [120, 180],
      "checksum": "blake3:..."
    }
  ]
}
```

QZT only guarantees the referenced original text can be retrieved and verified.

Evidence Ref range convention:

```text
byte_range: [start, end) using 0-based half-open byte offsets
line_range: [start, end) using 0-based half-open internal line numbers
```

User-facing CLIs MAY display line ranges as 1-based inclusive ranges, but stored Evidence Refs SHOULD use the internal half-open convention above.

---

## 4. Terms

| Term | Meaning |
|---|---|
| Container | Entire `.qzt` file |
| Chunk | A contiguous region of original input bytes |
| Compressed Chunk | A Chunk encoded as an independent zstd frame |
| Logical Offset | 0-based byte offset in the original input byte stream |
| Physical Offset | 0-based byte offset in the `.qzt` file |
| Line | Byte range terminated by newline sequence or EOF |
| Line Start | Logical offset of the first byte of a line |
| Line End | Logical offset immediately after the line bytes, including newline if present |
| Chunk Table | Required table mapping logical byte ranges to compressed chunk locations |
| Line Index | Sparse or dense data allowing line-number access |
| Index Root | Directory describing locations/checksums/codecs of index blocks |
| Footer Trailer | Fixed 64-byte trailer at EOF that makes footer discoverable |
| Footer Payload | Variable-length canonical CBOR payload referenced by Footer Trailer |
| Evidence Ref | Stable pointer from an external system into QZT original text |
| Human View | Text produced by partial decode for user display |
| Search Granule | Small original-text range used as a posting target in a search index |
| Posting List | Sorted list of Search Granule IDs for one token or n-gram |
| Search Manifest | Directory describing search index granules, dictionaries, postings, and planner metadata |

---

## 5. Normative language

This document uses the following normative terms:

```text
MUST       required
MUST NOT   prohibited
SHOULD     strongly recommended
SHOULD NOT discouraged
MAY        optional
```

When Japanese prose appears around these words, the English normative word remains controlling.

---

## 6. File overview

A QZT v0.1 Core file has this logical structure:

```text
+-------------------------------+
| Fixed Header, 128 bytes       |
+-------------------------------+
| Compressed Chunk 0            |
+-------------------------------+
| Compressed Chunk 1            |
+-------------------------------+
| Compressed Chunk 2            |
+-------------------------------+
| ...                           |
+-------------------------------+
| Metadata Block                |
+-------------------------------+
| Optional Dictionary Block(s)  |
+-------------------------------+
| Chunk Table Block             |
+-------------------------------+
| Optional Dense Line Index     |
+-------------------------------+
| Optional Extension Blocks     |
+-------------------------------+
| Index Root                    |
+-------------------------------+
| Footer Payload                |
+-------------------------------+
| Footer Trailer, 64 bytes      |
+-------------------------------+
```

The physical order of metadata/index blocks MAY differ, as long as:

```text
- Header points to Metadata Block
- Footer Trailer points to Footer Payload
- Footer Payload points to Index Root
- Index Root points to Chunk Table and all index blocks
```

Compressed chunks do not require per-chunk headers. Their locations and sizes are authoritative in the Chunk Table.

### 6.1 Physical range model

All physical byte ranges in a QZT file are 0-based half-open ranges:

```text
[offset, offset + size)
```

Readers MUST validate that `offset + size` does not overflow `u64`.

Readers MUST validate that every referenced physical range is inside:

```text
[0, final_file_size)
```

The following physical ranges are reserved:

```text
Header:         [0, 128)
Footer Trailer: [final_file_size - 64, final_file_size)
Footer Payload: [footer_payload_offset, footer_payload_offset + footer_payload_size)
Metadata:       [metadata_offset, metadata_offset + metadata_size)
Index Root:     [index_root.offset, index_root.offset + index_root.size)
Chunk Table:    block descriptor range for type "chunk_table"
Compressed chunks: Chunk Table physical ranges
```

Required ranges MUST NOT overlap, except that an optional `metadata` block descriptor in Index Root MAY describe the same Metadata range already referenced by Header and Footer Payload.

Compressed chunk ranges MUST NOT overlap any metadata, index, footer payload, footer trailer, or other compressed chunk range.

Writers SHOULD emit blocks in the logical order shown in Section 6, but readers MUST rely on validated offsets and sizes, not physical order.

---

## 7. Byte order and primitive types

All integer fields in fixed binary structures MUST be little-endian.

Primitive names:

| Name | Size | Meaning |
|---|---:|---|
| u8 | 1 | unsigned 8-bit integer |
| u16 | 2 | unsigned 16-bit integer |
| u32 | 4 | unsigned 32-bit integer |
| u64 | 8 | unsigned 64-bit integer |
| i32 | 4 | signed 32-bit integer |
| bstr | variable | byte string |
| bstr[N] | N | byte string of exactly N bytes |

All unspecified/reserved bytes MUST be zero when written.  
Readers MUST reject non-zero reserved bytes unless a future compatible version defines them.

### 7.1 Deterministic CBOR profile

When this document says "canonical CBOR", it means the QZT deterministic CBOR profile.

QZT deterministic CBOR MUST follow these rules:

```text
- definite-length arrays, maps, byte strings, and text strings only
- shortest valid integer encoding
- no floating point values in Core schemas
- no CBOR tags in Core schemas
- no duplicate map keys
- map keys sorted by their encoded byte-string order
- text string keys encoded as UTF-8
- byte strings used for bstr16 and bstr32 values exactly match the required length
```

Readers MUST reject non-deterministic CBOR encodings for Footer Payload, Metadata, Index Root, and all Core-defined CBOR extension blocks.

Readers MUST reject missing required fields.
Readers MUST reject duplicate map keys.
Readers MUST ignore unknown fields only when the surrounding schema explicitly allows extension fields.

The logical schemas in this document use YAML notation for readability. The serialized representation is deterministic CBOR, not YAML or JSON.

Core CBOR schemas are closed by default:

```text
- required fields MUST be present
- optional fields MAY be absent
- unknown fields MUST be rejected unless the schema explicitly says they are allowed
```

Footer Payload optional fields are limited to the fields listed in Section 10 for v0.1.

Index Root MAY contain unknown optional block descriptors. Core block descriptors MUST use the fields defined in Section 18. Extension specs MAY define additional descriptor fields for their own block types.

---

## 8. Fixed Header

The file starts with a fixed 128-byte header.

```text
Offset  Size  Type       Field
0       8     bstr[8]    magic = "QZT\0TXT1"
8       2     u16        major_version = 0
10      2     u16        minor_version = 1
12      4     u32        header_length = 128
16      8     u64        header_flags
24      8     u64        metadata_offset
32      8     u64        metadata_size
40      8     u64        index_hint_offset, 0 if unknown
48      16    bstr[16]   container_id
64      64    bstr[64]   reserved_zero
```

### 8.1 Header fields

`magic` MUST be exactly:

```text
QZT\0TXT1
```

`container_id` MUST be a random or content-derived 128-bit identifier.  
Implementations SHOULD use UUIDv4 bytes or BLAKE3-derived 128-bit truncated bytes.

`header_flags` has no defined bits in v0.1 and MUST be `0`.
Readers MUST reject non-zero `header_flags` for v0.1 files.

`metadata_offset` and `metadata_size` MUST point to the canonical CBOR Metadata Block.

`index_hint_offset` MAY point to Index Root for faster loading. If it is `0`, readers MUST use the Footer Trailer.

`index_hint_offset` is not authoritative. Readers MAY use it as a fast path only after validating the Footer Trailer, Footer Payload, and Index Root checksum.

If `index_hint_offset` is non-zero but outside the file, points to a malformed object, points to an Index Root whose checksum does not match the validated Footer Payload, or otherwise fails validation, readers MUST ignore the hint and continue by using the Footer Trailer path. An invalid hint MUST NOT make an otherwise valid container invalid. Readers MUST report an error only when the authoritative Footer Trailer, Footer Payload, or Index Root path is invalid.

The Header MAY be written as a placeholder at the start and patched at `finish()` time.

### 8.2 Version handling

A v0.1 Reader Core implementation MUST accept only:

```text
major_version = 0
minor_version = 1
```

Readers MUST return `UnsupportedVersion` for any other Header or Footer Trailer version unless the implementation explicitly supports that version.

Header, Footer Trailer, Footer Payload, Metadata, and Index Root format versions MUST all agree for v0.1 files.

---

## 9. Footer Trailer

The last 64 bytes of every QZT v0.1 Core file MUST be the Footer Trailer.

```text
Offset from trailer start  Size  Type       Field
0                          8     bstr[8]    trailer_magic = "QZTTAIL1"
8                          2     u16        major_version = 0
10                         2     u16        minor_version = 1
12                         4     u32        trailer_length = 64
16                         8     u64        footer_payload_offset
24                         8     u64        footer_payload_size
32                         32    bstr[32]   footer_payload_checksum_blake3
```

The trailer starts at:

```text
file_size - 64
```

The checksum is BLAKE3-256 of the exact Footer Payload bytes.

### 9.1 Reader open procedure

A reader MUST open a QZT file as follows:

```text
1. Read first 128 bytes.
2. Verify Header magic and version.
3. Seek to EOF - 64.
4. Read Footer Trailer.
5. Verify trailer_magic and trailer_length.
6. Read Footer Payload using footer_payload_offset and footer_payload_size.
7. Verify footer_payload_checksum_blake3.
8. Parse Footer Payload as canonical CBOR.
9. Verify final_file_size equals the actual file size.
10. Verify Header container_id equals Footer Payload container_id.
11. Verify Header metadata_offset/metadata_size equals Footer Payload metadata offset/size.
12. Read Metadata referenced by Footer Payload.
13. Verify Metadata checksum and parse Metadata as canonical CBOR.
14. Verify Metadata container_id equals Header/Footer container_id.
15. Read Index Root referenced by Footer Payload.
16. Verify Index Root checksum.
17. Parse Index Root as canonical CBOR.
18. Verify Index Root container_id equals Header/Footer/Metadata container_id.
19. Verify Metadata source fields equal Index Root content fields where both are present:
    original_size, original_checksum, line_count.
20. Read Chunk Table referenced by Index Root.
21. Verify Chunk Table checksum.
```

If any required step fails, reader MUST return an error.

---

## 10. Footer Payload

Footer Payload MUST be canonical CBOR.

Required logical schema:

```yaml
schema: "qzt.footer.v1"
format_version: [0, 1]
container_id: bstr16
index_root:
  offset: u64
  size: u64
  checksum:
    algorithm: "blake3"
    value: bstr32
metadata:
  offset: u64
  size: u64
  checksum:
    algorithm: "blake3"
    value: bstr32
final_file_size: u64
footer_flags: u64
```

`final_file_size` MUST equal the actual file size in bytes.

`footer_flags` has no defined bits in v0.1 and MUST be `0`.
Readers MUST reject non-zero `footer_flags` for v0.1 files.

Footer Payload MAY include optional fields:

```yaml
created_at_unix_ms: u64
writer: string
container_checksum:
  algorithm: "blake3"
  value: bstr32
```

`container_checksum`, if present, MUST be BLAKE3-256 over the exact file bytes from offset `0` up to, but not including, `footer_payload_offset`.

Footer Payload and Footer Trailer are not included in `container_checksum`; they are covered by `footer_payload_checksum_blake3` and required structural checks. This avoids circular checksums and preserves streaming writer behavior.

---

## 11. Metadata Block

Metadata Block MUST be canonical CBOR.

Required logical schema:

```yaml
schema: "qzt.metadata.v1"
format: "qzt"
format_version: [0, 1]
container_id: bstr16

identity:
  name: string | null
  profile: "minimal" | "core" | "log" | "archive" | "memory"
  created_by: string
  created_at_unix_ms: u64 | null

source:
  media_type: "text"
  encoding: "utf-8"
  original_size: u64
  original_checksum:
    algorithm: "blake3"
    value: bstr32
  newline_mode: "lf" | "crlf" | "mixed" | "none"
  line_count: u64

compression:
  codec: "zstd"
  zstd_level: i32
  independent_frames: true
  zstd_frame_checksum: bool
  dictionary_mode: "none" | "embedded"

chunking:
  target_chunk_size: u64
  max_chunk_size: u64
  boundary: "line-preferred" | "document-then-line" | "byte-window"
  utf8_boundary_required: true

indexes:
  chunk_table: true
  sparse_line_index: true
  dense_line_index: bool
  document_index: bool
  token_index: bool
  ngram_index: bool
  vector_index: false

integrity:
  compressed_chunk_checksum: "blake3"
  uncompressed_chunk_checksum: "blake3"
  index_checksum: "blake3"

compatibility:
  qzt_is_zst_stream: false
  chunks_are_independent_zstd_frames: true
```

QZT v0.1 Core MUST set:

```yaml
source.encoding: "utf-8"
compression.codec: "zstd"
compression.independent_frames: true
chunking.utf8_boundary_required: true
compatibility.qzt_is_zst_stream: false
compatibility.chunks_are_independent_zstd_frames: true
```

`compression.zstd_level` MUST be a concrete signed integer. Profile names such as `"high"` are writer presets and MUST NOT be serialized in Metadata.

### 11.1 Metadata and original text

Metadata MUST NOT contain transformed text that replaces the original payload.  
Any normalized text for search MUST be part of optional search indexes, never the source of truth.

Human View MUST always be generated from original bytes after partial decode.

---

## 12. UTF-8 and text boundary rules

QZT v0.1 Core is UTF-8 text only.

### 12.1 Chunk boundary

Chunk boundaries MUST occur at valid UTF-8 code point boundaries.

Writers MUST NOT split between CR and LF in a CRLF sequence, regardless of chunking boundary mode.

When `boundary = "line-preferred"`:

```text
- writer SHOULD split at newline boundaries near target_chunk_size
- if a line exceeds max_chunk_size, writer MUST split inside the line
- even forced splits MUST preserve UTF-8 code point boundaries
```

`max_chunk_size` is a hard writer limit for uncompressed chunk bytes. A writer MUST NOT emit an uncompressed chunk larger than `max_chunk_size`, except that it MUST fail with `ResourceLimitExceeded` if no valid UTF-8 boundary exists within the limit.

If input is not valid UTF-8, writer MUST fail with `InvalidUtf8`, unless a future non-core profile explicitly supports binary or legacy encodings.

### 12.2 Logical offset

All Logical Offsets are:

```text
0-based byte offsets in the original input byte stream.
```

They are NOT character offsets, Unicode scalar offsets, or grapheme offsets.

### 12.3 Text range API

`read_range(offset, length)` returns raw bytes and MAY start/end at any byte offset.

`read_text_range(offset, length)` MUST validate that both start and end are UTF-8 boundaries. If not, it MUST return `InvalidUtf8Boundary`.

A convenience API MAY provide `read_text_window(offset, length, mode)` that expands to nearest UTF-8 boundaries.

---

## 13. Line semantics

### 13.1 Line definition

A line is a byte range that ends with a newline sequence or EOF.

Recognized newline sequences:

```text
LF   = 0x0A
CRLF = 0x0D 0x0A
```

A CR byte not followed by LF is treated as ordinary data in v0.1.

A final newline does not create an additional empty line.

Examples:

| Input bytes | line_count |
|---|---:|
| `""` | 0 |
| `"a"` | 1 |
| `"a\n"` | 1 |
| `"a\nb"` | 2 |
| `"a\nb\n"` | 2 |
| `"\n"` | 1 |
| `"\n\n"` | 2 |

Line numbers are assigned by Line Start order.

For a non-empty file, line `0` starts at logical offset `0`.
Each subsequent line starts immediately after a recognized newline sequence.
A final newline does not create a Line Start at EOF.

### 13.2 Newline mode

`source.newline_mode` is derived from the original byte stream:

```text
"none"  no recognized LF or CRLF newline sequence appears
"lf"    at least one LF newline appears and no CRLF newline appears
"crlf"  at least one CRLF newline appears and no standalone LF newline appears
"mixed" both LF and CRLF newline sequences appear
```

The LF byte inside a CRLF sequence MUST NOT also count as a standalone LF newline.

Deep verify MUST recompute `newline_mode` from the reconstructed original bytes and compare it to Metadata.

### 13.3 Line numbering

Internal APIs and index fields MUST use 0-based line numbers.

CLI commands MUST use 1-based line numbers by default.

Example:

```bash
qzt line data.qzt 1000
```

means internal line number `999`.

CLI MAY support:

```bash
qzt line data.qzt 999 --zero-based
```

### 13.4 Line output

Reader APIs SHOULD provide both exact and display modes:

```text
read_line_raw(line)       -> exact original line bytes, including newline if present
read_line_text(line)      -> exact bytes decoded as UTF-8
read_line_display(line)   -> text suitable for terminal display
```

The `qzt line` CLI SHOULD default to exact original bytes.

---

## 14. Chunks

Each Chunk MUST be encoded as an independent zstd frame.

The following invariant MUST hold:

```text
decompress(chunk_0)
+ decompress(chunk_1)
+ ...
+ decompress(chunk_n)
== original_input_bytes
```

Chunks MUST be ordered by increasing `logical_offset`.

Chunks MUST NOT overlap.

There MUST be no gap in the reconstructed logical byte stream:

```text
chunk[i].logical_offset + chunk[i].uncompressed_size
== chunk[i+1].logical_offset
```

for all adjacent chunks.

### 14.1 Zstd frame requirements

Each compressed chunk MUST contain exactly one complete Zstandard frame.

The zstd frame MAY omit the decompressed content size. QZT readers MUST treat the Chunk Table `uncompressed_size` as authoritative.

During decompression, readers MUST stop and return `ZstdDecodeError` or `ResourceLimitExceeded` if decoded output exceeds the declared `uncompressed_size` or configured resource limits.

After decompression, the decoded output size MUST equal the Chunk Table `uncompressed_size`.

If `compression.zstd_frame_checksum = true`, zstd's frame checksum MAY provide an additional codec-level check. QZT BLAKE3 chunk checksums remain authoritative for QZT verification.

---

## 15. Dictionary handling

QZT v0.1 Core MAY use embedded zstd dictionaries.

If a chunk requires a dictionary:

```text
- Chunk Table entry MUST reference dictionary_id
- Index Root MUST include a Dictionary Block descriptor
- Dictionary Block MUST contain the referenced dictionary
```

External dictionaries are NOT allowed in QZT v0.1 Core because they weaken long-term evidence restoration.

`dictionary_id = 0` means no dictionary.

Multiple dictionaries are allowed by the logical model, but reference implementations SHOULD start with either:

```text
- no dictionary
- one embedded dictionary
```

The Dictionary Block descriptor in Index Root MUST use:

```yaml
type: "dictionary"
required: false
codec: "qzt-dict-cbor-v1"
```

`qzt-dict-cbor-v1` payload is deterministic CBOR:

```yaml
schema: "qzt.dictionary.v1"
format_version: [0, 1]
container_id: bstr16
dictionaries:
  - dictionary_id: u32
    codec: "zstd"
    bytes: bstr
    checksum:
      algorithm: "blake3"
      value: bstr32
```

`dictionary_id` values in Dictionary Block MUST be unique and MUST be greater than `0`.

Every non-zero Chunk Table `dictionary_id` MUST resolve to exactly one Dictionary Block entry.

The Dictionary Block descriptor uses `required: false` because dictionaries are optional for the format. If any chunk references a non-zero `dictionary_id`, the referenced Dictionary Block is required for decoding that container.

Readers MUST reject a container with a missing dictionary, duplicate dictionary ID, checksum mismatch, or dictionary codec other than `"zstd"`.

Writer Core MAY choose not to emit dictionaries. If a writer emits any non-zero `dictionary_id`, it MUST emit a valid Dictionary Block.

---

## 16. Chunk Table

Chunk Table is required.

Index Root MUST contain one required Chunk Table block descriptor. Its identifying fields are:

```yaml
type: "chunk_table"
required: true
codec: "qzt-ctbl-fixed-v1"
```

The descriptor MUST also include the full block descriptor fields defined in Section 18: `offset`, `size`, `checksum`, and `flags`.

### 16.1 Logical Chunk Entry fields

Each Chunk Entry MUST include:

```yaml
chunk_id: u64
physical_offset: u64
compressed_size: u64
logical_offset: u64
uncompressed_size: u64
first_line: u64
line_count: u64
dictionary_id: u32
flags: u32
compressed_checksum_blake3: bstr32
uncompressed_checksum_blake3: bstr32
```

`first_line` is the 0-based line number of the first line whose Line Start is inside this chunk.

`line_count` is the number of Line Starts inside this chunk.

If no Line Start occurs inside a chunk, `line_count` MUST be `0`, and `first_line` MUST equal the number of Line Starts before this chunk.

The sum of all Chunk Entry `line_count` values MUST equal the container `line_count`.

### 16.2 Fixed binary record

`qzt-ctbl-fixed-v1` encodes each entry as a fixed 128-byte record:

```text
Offset  Size  Type       Field
0       8     u64        chunk_id
8       8     u64        physical_offset
16      8     u64        compressed_size
24      8     u64        logical_offset
32      8     u64        uncompressed_size
40      8     u64        first_line
48      8     u64        line_count
56      4     u32        dictionary_id
60      4     u32        flags
64      32    bstr[32]   compressed_checksum_blake3
96      32    bstr[32]   uncompressed_checksum_blake3
```

Records MUST be sorted by `chunk_id`.  
`chunk_id` MUST start at `0` and increase by `1`.

The first entry MUST have:

```text
logical_offset = 0
first_line = 0
```

If the file is empty, Chunk Table MAY contain zero records.

### 16.3 Chunk Table block invariants

The Chunk Table block size MUST be:

```text
chunk_count * 128
```

`chunk_count` is the `content.chunk_count` value stored in Index Root.

For an empty original input:

```text
source.original_size = 0
source.line_count = 0
Index Root content.chunk_count = 0
Chunk Table block size = 0
```

For non-empty original input, Chunk Table MUST contain at least one record.

Writers MUST NOT emit zero-length chunks.
Readers MUST reject any Chunk Entry where `compressed_size = 0` or `uncompressed_size = 0`.

The sum of all Chunk Entry `uncompressed_size` values MUST equal `source.original_size`.

The final Chunk Entry MUST satisfy:

```text
logical_offset + uncompressed_size == source.original_size
```

For every adjacent pair of Chunk Entries:

```text
chunk[i + 1].first_line == chunk[i].first_line + chunk[i].line_count
```

This first-line continuity check uses declared `line_count` values and does not require decompression. Deep verify MUST additionally recompute Line Starts from decoded bytes.

### 16.4 Chunk flags

Chunk Entry `flags` is a bitset.

Defined v0.1 bits:

```text
bit 0: starts_with_line_continuation
```

If `starts_with_line_continuation` is set, the first byte of the uncompressed chunk is a continuation of a line that started in an earlier chunk.

Writers MUST set `starts_with_line_continuation` when:

```text
logical_offset > 0
and the previous original byte is not LF (0x0A)
```

Because writers MUST NOT split between CR and LF, checking the previous byte for LF is sufficient for v0.1 line-start detection.

No other chunk flag bits are defined in v0.1.
Writers MUST write all unknown flag bits as zero.
Readers MUST reject non-zero unknown flag bits.

### 16.5 Checksums

`compressed_checksum_blake3` is computed over the exact compressed zstd frame bytes stored in the QZT file.

`uncompressed_checksum_blake3` is computed over the decompressed original chunk bytes.

Compressed checksum can be verified without decompression.  
Uncompressed checksum requires decompression.

---

## 17. Line Index

QZT v0.1 Core requires at least a sparse line index.

Sparse line index is provided by the Chunk Table fields:

```text
first_line
line_count
```

These fields index Line Starts, not newline terminators.

A reader can resolve a line as:

```text
1. Binary search Chunk Table for chunk where:
   first_line <= target_line < first_line + line_count
2. Read and decompress that chunk.
3. Locate the target Line Start inside the chunk.
4. Scan forward until LF, CRLF, or EOF.
5. If the line does not end in the starting chunk, continue into adjacent chunks until the line terminator or EOF.
6. Return the requested line.
```

Chunks with `line_count = 0` are never selected by step 1, but readers may need to decode them when a line started in an earlier chunk continues through them.

When scanning a chunk with `starts_with_line_continuation` set, bytes before the first recognized newline sequence are a continuation fragment and MUST NOT be counted as a Line Start.

### 17.1 Dense Line Index extension

A Dense Line Index MAY be present for faster in-chunk line access.

Index Root descriptor:

```yaml
type: "dense_line_index"
required: false
codec: "qzt-line-delta-varint-v1"
```

Logical dense entry:

```yaml
chunk_id: u64
line_start_offsets: delta-varint[u64]
```

`line_start_offsets` are byte offsets within the uncompressed chunk, pointing to each Line Start contained in that chunk.

The number of `line_start_offsets` entries for a chunk MUST equal that chunk's Chunk Table `line_count`.

The offsets MUST be strictly increasing and MUST be less than `uncompressed_size`.

`qzt-line-delta-varint-v1` payload layout:

```text
dense_line_index_payload:
  entry_count: varuint
  entries: dense_entry[entry_count]

dense_entry:
  chunk_id: varuint
  offset_count: varuint
  line_start_offset_deltas: varuint[offset_count]
```

`varuint` is unsigned LEB128 with minimal encoding. Entries MUST be sorted by `chunk_id` and, for v0.1 reference containers, MUST contain exactly one entry for every Chunk Table entry. `line_start_offset_deltas` encodes the first offset as a delta from `0`, then every subsequent offset as `current_offset - previous_offset`.

Dense Line Index stores starts, not ends. A reader still determines the exact Line End by scanning for LF, CRLF, or EOF, continuing into adjacent chunks if required.

Dense Line Index MUST NOT be treated as source of truth. If it disagrees with the decoded chunk during deep verify, the container MUST be reported corrupt.

---

## 18. Index Root

Index Root MUST be canonical CBOR.

It is a block directory that tells readers where to find required and optional blocks.

Required logical schema:

```yaml
schema: "qzt.index-root.v1"
format_version: [0, 1]
container_id: bstr16
blocks:
  - type: string
    required: bool
    offset: u64
    size: u64
    codec: string
    checksum:
      algorithm: "blake3"
      value: bstr32
    flags: u64
content:
  original_size: u64
  original_checksum:
    algorithm: "blake3"
    value: bstr32
  chunk_count: u64
  line_count: u64
```

Required block descriptors for QZT v0.1 Core:

```yaml
- type: "chunk_table"
  required: true
  offset: u64
  size: u64
  codec: "qzt-ctbl-fixed-v1"
  checksum:
    algorithm: "blake3"
    value: bstr32
  flags: 0
```

Optional block descriptors MAY include:

```yaml
- type: "dense_line_index"
- type: "dictionary"
- type: "document_index"
- type: "token_index"
- type: "ngram_index"
- type: "optimizer_metadata"
- type: "extension"
```

Readers MUST ignore unknown optional blocks.

Readers MUST fail if an unknown `required: true` block is present.

Block descriptor `flags` has no defined bits in v0.1 and MUST be `0`.
Readers MUST reject non-zero block descriptor `flags` for v0.1 files.

Block descriptors MUST NOT overlap each other, compressed chunk byte ranges, Footer Payload, or Footer Trailer.

---

## 19. Block type registry

Initial block type names:

| Type name | Required in Core | Purpose |
|---|---:|---|
| `metadata` | yes, referenced by Header | container metadata |
| `chunk_table` | yes | compressed chunk directory |
| `dense_line_index` | no | faster in-chunk line lookup |
| `dictionary` | no | embedded zstd dictionary |
| `document_index` | no | doc_id to byte/line ranges |
| `token_index` | no | lexical candidate search |
| `ngram_index` | no | substring candidate search |
| `optimizer_metadata` | no | encoder parameter notes only |
| `extension` | no | future extension |

Metadata is directly referenced by Header and SHOULD also be represented in Index Root for diagnostics.

---

## 20. Read algorithms

### 20.1 Export all

```text
for chunk in chunk_table ordered by chunk_id:
    read compressed bytes
    verify compressed checksum if requested
    decompress zstd frame
    verify uncompressed checksum if requested
    append original bytes to output
```

Result MUST equal original input bytes.

### 20.2 Read byte range

Input:

```text
offset: u64
length: u64
```

Procedure:

```text
1. Validate offset + length does not overflow u64.
2. Validate offset + length <= original_size.
3. If length = 0, return an empty byte string without reading chunks.
4. Find first chunk overlapping [offset, offset + length).
5. Decompress only overlapping chunks.
6. Slice decoded chunk bytes by logical offset.
7. Concatenate slices.
8. Return exactly length bytes.
```

### 20.3 Read text range

Same as byte range, plus:

```text
- validate start offset is UTF-8 boundary
- validate end offset is UTF-8 boundary
- decode as UTF-8
```

If validation fails, return `InvalidUtf8Boundary`.

### 20.4 Read line

Input internal line number is 0-based.

```text
1. Validate line < line_count.
2. Find the starting chunk using first_line and line_count.
3. Decompress chunk.
4. Locate the Line Start inside chunk by scan or Dense Line Index.
5. Scan forward to LF, CRLF, or EOF.
6. Continue into adjacent chunks if the Line End is not in the starting chunk.
7. Return exact original bytes for that line.
```

If a line spans multiple chunks because it exceeds `max_chunk_size`, reader MUST continue into adjacent chunks until line terminator or EOF. Writer SHOULD avoid this when possible, but reader MUST support it.

---

## 21. Verification

QZT defines three verification levels.

### 21.1 quick verify

Quick verify MUST check:

```text
- Header magic/version/length
- Header reserved bytes are zero
- Header flags are known and valid
- Footer Trailer magic/version/length
- Footer Payload checksum
- Footer Payload parse
- final_file_size equals actual file size
- Footer Payload flags are known and valid
- Header/Footer container_id consistency
- Header/Footer/Payload/Metadata/Index Root version consistency
- Header/Footer metadata offset and size consistency
- Metadata checksum
- Metadata parse
- Metadata/Header/Footer container_id consistency
- Index Root checksum
- Index Root parse
- Index Root/Header/Footer/Metadata container_id consistency
- Metadata source fields equal Index Root content fields:
    original_size, original_checksum, line_count
- required block descriptors exist
- block descriptor flags are known and valid
- block descriptor physical ranges are inside file and do not overlap
- Chunk Table block checksum
- Chunk Table structural consistency:
    block size equals chunk_count * 128
    chunk_count equals number of records
    chunk_id sequence
    logical offset continuity
    first_line continuity by declared line_count values
    sum(line_count) equals container line_count
    sum(uncompressed_size) equals original_size
    no zero-length chunks in non-empty files
    physical ranges inside file
    no overlap
    no out-of-bounds reads
    chunk flags are known and valid
```

Quick verify MUST NOT require decompressing all chunks.

### 21.2 normal verify

Normal verify MUST perform quick verify plus:

```text
- container_checksum, if present, over bytes [0, footer_payload_offset)
- checksum of all index blocks
- compressed_checksum_blake3 for all chunks
- dictionary block checksums if present
- optional block checksums for all known optional blocks
```

Normal verify MUST NOT require full decompression.

### 21.3 deep verify

Deep verify MUST perform normal verify plus:

```text
- decompress every chunk
- verify every uncompressed_checksum_blake3
- verify reconstructed total size
- verify reconstructed original_checksum
- verify UTF-8 validity of reconstructed stream
- verify line_count
- verify newline_mode
- verify Chunk Table first_line and line_count against decoded Line Starts
- verify starts_with_line_continuation flags against decoded chunk boundaries
- verify Dense Line Index if present
- verify Document Index if present
```

Deep verify proves that the QZT file can restore the original byte stream.

When verifying `starts_with_line_continuation`, an implementation MAY need decoded boundary bytes from adjacent chunks. This verification SHOULD be implemented as a single forward pass over decoded chunks, or an equivalent bounded-cache algorithm, so long lines spanning many chunks remain O(total uncompressed bytes + chunk_count) rather than causing repeated adjacent-chunk decompression.

---

## 22. Immutability

A QZT v0.1 Core container is immutable after `finish()`.

Writers MUST NOT append or modify chunks/indexes in place after final Footer Trailer is written.

Updates MUST create a new file through operations such as:

```bash
qzt repack old.qzt -o new.qzt
qzt merge a.qzt b.qzt -o merged.qzt
qzt compact old.qzt -o compacted.qzt
```

These commands are examples of future maintenance operations. They are not required for QZT v0.1 Core CLI conformance unless a reference implementation phase explicitly includes them. v0.1 Core CLI conformance requires `pack`, `info`, `export`, `range`, `line`, and `verify`.

Future versions MAY define appendable segment containers, but v0.1 does not.

---

## 23. Error codes

Implementations SHOULD expose at least these errors:

```text
InvalidMagic
UnsupportedVersion
InvalidHeader
InvalidFooterTrailer
InvalidFooterPayload
NonCanonicalCbor
DuplicateCborKey
FooterChecksumMismatch
FinalFileSizeMismatch
ContainerIdMismatch
MetadataChecksumMismatch
MetadataInvalid
VersionMismatch
NewlineModeMismatch
IndexRootChecksumMismatch
MissingRequiredBlock
UnknownRequiredBlock
InvalidFlags
ChunkTableChecksumMismatch
ChunkTableInvalid
ChunkCountMismatch
ChunkSizeMismatch
PhysicalRangeOutOfBounds
LogicalRangeOutOfBounds
InvalidUtf8
InvalidUtf8Boundary
LineOutOfRange
MissingDictionary
DictionaryChecksumMismatch
CompressedChunkChecksumMismatch
UncompressedChunkChecksumMismatch
ZstdDecodeError
ContainerCorrupt
ResourceLimitExceeded
```

---

## 24. CLI specification

### 24.1 pack

```bash
qzt pack input.txt -o output.qzt
```

Options:

```bash
--profile minimal|core|log|archive|memory
--zstd-level N
--chunk-size SIZE
--max-chunk-size SIZE
--dict none|auto|PATH
--checksum blake3
--dense-line-index on|off
```

Writer option validation:

```text
- target chunk size MUST be greater than 0
- max chunk size MUST be greater than 0
- target chunk size MUST be less than or equal to max chunk size
- max chunk size SHOULD be at least 4 bytes for UTF-8 Core writers
- zstd level MUST be accepted by the implementation's zstd encoder
- checksum algorithm MUST be blake3 for v0.1 Core
- --dict PATH MUST embed the dictionary bytes in the QZT file
```

Required behavior:

```text
- validate input is UTF-8
- split on UTF-8 boundaries
- prefer line boundaries
- write independent zstd frames
- write Metadata
- write Chunk Table
- write Index Root
- write Footer Payload
- write Footer Trailer
- patch Header
```

### 24.2 info

```bash
qzt info data.qzt
```

SHOULD print:

```text
Format: QZT 0.1
Profile: core
Original size: ...
Compressed size: ...
Chunks: ...
Lines: ...
Compression: zstd level ...
Chunk target: ...
Line index: sparse|dense
Document index: yes|no
Token index: yes|no
Ngram index: yes|no
Vector index: no
Checksum: blake3
Zstd stream compatible: no
```

### 24.3 export

```bash
qzt export data.qzt -o restored.txt
```

MUST output original bytes exactly.

### 24.4 range

```bash
qzt range data.qzt --bytes 1048576:2097152
qzt range data.qzt --lines 1000:1200
```

CLI byte ranges use 0-based half-open byte offsets.

Recommended interpretation:

```text
--bytes A:B means original bytes [A, B), where A is inclusive and B is exclusive.
```

CLI line ranges are 1-based and inclusive by default unless implementation explicitly documents otherwise.

Recommended interpretation:

```text
--lines A:B means lines A through B inclusive using 1-based user numbering.
```

### 24.5 line

```bash
qzt line data.qzt 1000
qzt line data.qzt 999 --zero-based
```

Default is 1-based.

### 24.6 verify

```bash
qzt verify data.qzt --quick
qzt verify data.qzt --normal
qzt verify data.qzt --deep
```

Default SHOULD be `--normal`.

---

## 25. Reader API

Conceptual Rust-style API:

```rust
struct QztReader;

impl QztReader {
    fn open(path: &Path) -> Result<QztReader>;

    fn info(&self) -> Result<QztInfo>;

    fn read_range(&self, offset: u64, length: u64) -> Result<Vec<u8>>;

    fn read_text_range(&self, offset: u64, length: u64) -> Result<String>;

    fn read_line_raw(&self, line_zero_based: u64) -> Result<Vec<u8>>;

    fn read_line_text(&self, line_zero_based: u64) -> Result<String>;

    fn export_to<W: Write>(&self, writer: W) -> Result<()>;

    fn verify(&self, level: VerifyLevel) -> Result<VerifyReport>;
}
```

---

## 26. Writer API

Conceptual Rust-style API:

```rust
struct QztWriter;

impl QztWriter {
    fn create(path: &Path, options: QztWriteOptions) -> Result<QztWriter>;

    fn write_all(&mut self, bytes: &[u8]) -> Result<()>;

    fn finish(self) -> Result<QztSummary>;
}
```

The writer MAY stream input, but it MUST be able to patch the Header at finish time or otherwise write a valid Header.

---

## 27. Profiles

### 27.1 minimal

For storage with exact restoration and byte-range access.

```yaml
profile: "minimal"
target_chunk_size: 4MiB
max_chunk_size: 16MiB
sparse_line_index: true
dense_line_index: false
document_index: false
token_index: false
ngram_index: false
```

For a QZT v0.1 Core container, minimal profile still MUST populate Chunk Table `first_line` and `line_count`, and a Reader Core implementation still MUST support line access.

A minimal-only non-Core tool MAY omit line CLI support, but it MUST NOT claim QZT v0.1 Reader Core conformance.

### 27.2 core

Reference default for QZT v0.1 Core.

```yaml
profile: "core"
target_chunk_size: 1MiB
max_chunk_size: 8MiB
boundary: "line-preferred"
sparse_line_index: true
dense_line_index: false
document_index: false
token_index: false
ngram_index: false
```

### 27.3 log

For logs.

```yaml
profile: "log"
target_chunk_size: 512KiB
max_chunk_size: 4MiB
boundary: "line-preferred"
sparse_line_index: true
dense_line_index: true
token_index: optional
ngram_index: optional
```

### 27.4 archive

For higher compression.

```yaml
profile: "archive"
target_chunk_size: 8MiB
max_chunk_size: 32MiB
boundary: "line-preferred"
sparse_line_index: true
dense_line_index: false
token_index: false
ngram_index: false
zstd_level: high
```

`zstd_level: high` is a profile preset. A writer MUST resolve it to a concrete signed integer `compression.zstd_level` in Metadata. The reference implementation SHOULD map `high` to zstd level 19 unless the user explicitly overrides the level.

### 27.5 memory

For Memory Pager / AI evidence systems.

```yaml
profile: "memory"
target_chunk_size: 256KiB
max_chunk_size: 2MiB
boundary: "document-then-line"
sparse_line_index: true
dense_line_index: true
document_index: true
token_index: optional
ngram_index: optional
vector_index: false
```

`memory` profile is an extension profile. It is not required for QZT v0.1 Core conformance.

---

## 28. Extension: Document Index

Document Index maps stable document IDs to original byte/line ranges.

Index Root descriptor:

```yaml
type: "document_index"
required: false
codec: "qzt-doc-index-cbor-v1"
```

`qzt-doc-index-cbor-v1` payload is deterministic CBOR:

```yaml
schema: "qzt.document-index.v1"
format_version: [0, 1]
container_id: bstr16
documents:
  - doc_id: string
    doc_id_hash: bstr16
    logical_offset: u64
    byte_length: u64
    first_line: u64
    line_count: u64
    chunk_start: u64
    chunk_end: u64
    checksum:
      algorithm: "blake3"
      value: bstr32
    metadata: map
```

Logical entry:

```yaml
doc_id: string
doc_id_hash: bstr16
logical_offset: u64
byte_length: u64
first_line: u64
line_count: u64
chunk_start: u64
chunk_end: u64
checksum:
  algorithm: "blake3"
  value: bstr32
metadata: map
```

`chunk_start` and `chunk_end` form a half-open chunk-id range `[chunk_start, chunk_end)`. The range MUST cover every Chunk Table entry whose logical byte range overlaps the document byte range. Empty document byte ranges MUST use `chunk_start == chunk_end`.

Document Index MUST be treated as an index into original text, not as a replacement for original text.

During deep verify, if Document Index exists, implementation SHOULD verify all document ranges are within original size and chunk ranges are consistent.

---

## 29. Extension: Search Index

Search Index is not QZT v0.1 Core.

Search Index is an acceleration structure. It MUST NOT replace the original text as the source of truth.

When provided, it MUST be a candidate index unless it explicitly declares complete semantics.

High-performance search in QZT depends on four separate mechanisms:

```text
1. use an index to produce a small set of candidate Search Granules
2. intersect postings before decoding compressed chunks
3. decode only chunks overlapping surviving candidates
4. verify every reported match against original bytes after partial decode
```

An implementation that only maps terms to whole chunks can be correct, but it SHOULD NOT claim high-performance search for large or high-cardinality corpora unless benchmark evidence shows acceptable candidate and decode costs.

### 29.1 Candidate search rule

Search flow:

```text
query
  -> search index
  -> candidate Search Granules
  -> candidate chunks
  -> partial decode
  -> verify match against original text
  -> return hits
```

Search indexes MAY have false positives.  
If `complete=false`, they MAY have false negatives.  
If `complete=true`, they MUST NOT have false negatives within the declared scope.

Search results MUST include enough information to retrieve and verify the original bytes:

```yaml
logical_offset: u64
byte_length: u64
chunk_start: u64
chunk_end: u64
score: float | null
source: "verified_original_bytes"
```

`score` is optional and MUST NOT affect evidence verification.

### 29.2 Search Granules

Search Index postings target Search Granules, not necessarily chunks.

Supported posting granularities:

```yaml
posting_granularity: "chunk" | "document" | "line" | "byte_window"
```

`chunk` granularity:

```text
- granule_id equals chunk_id
- no separate granule table is required
- simplest to implement
- may decode too much data for common terms
```

`document` granularity:

```text
- requires Document Index
- granule_id maps to a document range
- good for memory and archive profiles with natural document boundaries
```

`line` granularity:

```text
- granule_id maps to one logical line
- good for logs
- may produce large posting lists for common tokens
```

`byte_window` granularity:

```yaml
window_size: u64
window_overlap: u64
```

`byte_window` granularity is recommended for high-performance substring or n-gram search when no document boundaries exist.

For `byte_window`, `window_overlap` SHOULD be at least the maximum exact-match pattern length the index claims complete support for, or the index MUST declare `complete=false` for longer boundary-spanning matches.

For any granularity other than `chunk`, the Search Index MUST include a Granule Table:

```yaml
schema: "qzt.search-granules.v1"
granules:
  - granule_id: u64
    logical_offset: u64
    byte_length: u64
    chunk_start: u64
    chunk_end: u64
    first_line: u64 | null
    line_count: u64 | null
```

Granule ranges MUST be inside `source.original_size`.
`chunk_start` and `chunk_end` MUST identify all chunks overlapping the granule.
Granule IDs MUST start at `0`, increase by `1`, and be sorted by `logical_offset`.

### 29.3 Search Index physical model

Token and n-gram indexes SHOULD use the `qzt-search-block-v1` physical model.

Search Index descriptor:

```yaml
type: "token_index" | "ngram_index"
required: false
codec: "qzt-search-block-v1"
```

`qzt-search-block-v1` payload layout:

```text
Offset  Size  Type   Field
0       8     u64    manifest_size
8       N     cbor   Search Manifest, deterministic CBOR
8+N     ...   bytes  binary sections referenced by Search Manifest
```

All binary section offsets in Search Manifest are relative to the start of the `qzt-search-block-v1` payload.

Search Manifest schema:

```yaml
schema: "qzt.search-index.v1"
format_version: [0, 1]
container_id: bstr16
kind: "token" | "ngram"
source: "raw_utf8" | "normalized_utf8"
complete: bool
posting_granularity: "chunk" | "document" | "line" | "byte_window"
granule_count: u64
term_count: u64
tokenizer: map | null
ngram: map | null
boundary:
  mode: "none" | "adjacent_decode" | "adjacent_window_index"
  window_bytes: u64
planner:
  max_candidate_granules_default: u64
  max_decoded_bytes_default: u64
  high_df_per_million: u32
sections:
  granule_table:
    offset: u64
    size: u64
    codec: string
    checksum: { algorithm: "blake3", value: bstr32 }
  term_dictionary:
    offset: u64
    size: u64
    codec: string
    checksum: { algorithm: "blake3", value: bstr32 }
  postings:
    offset: u64
    size: u64
    codec: string
    checksum: { algorithm: "blake3", value: bstr32 }
  skip_data:
    offset: u64
    size: u64
    codec: string
    checksum: { algorithm: "blake3", value: bstr32 }
```

If `posting_granularity = "chunk"`, `granule_table.size` MAY be `0` and `granule_id` MUST equal `chunk_id`.

Search Manifest sections MUST NOT overlap and MUST be fully inside the Search Index block.

Readers MUST verify Search Manifest section checksums before trusting term dictionaries, postings, granules, or skip data.

### 29.4 Term Dictionary and postings

Term Dictionary entries MUST be sorted by lookup key.

Logical Term Dictionary entry:

```yaml
key: bstr
key_hash: bstr16
document_frequency: u64
granule_frequency: u64
posting_offset: u64
posting_size: u64
skip_offset: u64
skip_size: u64
flags: u64
```

`key` is the token bytes or n-gram bytes after applying the declared tokenizer/source rules.

`key_hash` SHOULD be BLAKE3-128 of `key` and MAY be used as a lookup accelerator. It MUST NOT replace exact `key` comparison.

If no Document Index is present, `document_frequency` MUST be `0`.

No Term Dictionary `flags` bits are defined in v0.1. Writers MUST write `flags = 0`; readers MUST reject non-zero unknown flag bits.

Posting lists MUST be sorted by increasing `granule_id`.

Recommended posting codec:

```yaml
posting_codec: "delta-varint-u64-v1"
```

`delta-varint-u64-v1` encodes:

```text
first granule_id as unsigned varint
then deltas from previous granule_id as unsigned varints
```

Skip data SHOULD include at least one skip point every 128 posting IDs for posting lists with at least 1024 entries.

Logical skip point:

```yaml
entry_index: u64
granule_id: u64
posting_byte_offset: u64
```

Skip data enables fast intersection without decoding entire high-frequency posting lists.

### 29.5 Token Index

Recommended default tokenizer for v0.1 search extension:

```text
ASCII alnum + "_" + "-" forms tokens.
All other code points are delimiters.
Japanese and other non-space languages should use char n-gram index.
```

Descriptor example:

```yaml
type: "token_index"
required: false
codec: "qzt-search-block-v1"
tokenizer:
  id: "qzt-simple-tokenizer-v1"
  lowercase: true
posting_granularity: "line" | "document" | "byte_window" | "chunk"
posting_codec: "delta-varint-u64-v1"
complete: true
```

For log search, `line` granularity is recommended.

For memory or document archives, `document` granularity is recommended if Document Index exists.

For broad text archives without document boundaries, `byte_window` granularity is recommended.

### 29.6 N-gram Index

Recommended:

```text
Japanese: char 2-gram or char 3-gram
Mixed logs: token index + char 3-gram
```

Descriptor example:

```yaml
type: "ngram_index"
required: false
codec: "qzt-search-block-v1"
n: 3
source: "raw_utf8" | "normalized_utf8"
posting_granularity: "byte_window" | "line" | "chunk"
complete: true
```

N-gram index builders MUST define whether n-grams are byte n-grams, Unicode scalar n-grams, or Unicode grapheme n-grams.

Recommended v0.1 default:

```yaml
unit: "unicode_scalar"
normalization: "none"
case_fold: false
```

If `source = "normalized_utf8"`, the index MUST store enough mapping metadata to verify candidate hits against original raw UTF-8 bytes.

### 29.7 Raw vs normalized indexes

Original text MUST NOT be normalized.

The first interoperable Search Extension MVP SHOULD implement `raw_utf8` indexes only. `normalized_utf8` indexes are a later extension and MUST NOT be added until their mapping metadata can prove every returned hit against original raw UTF-8 bytes.

Search extensions SHOULD separate:

```text
raw_token_index
normalized_token_index
raw_ngram_index
normalized_ngram_index
```

Human View MUST always return original text.

### 29.8 Boundary matches

Matches spanning chunk boundaries are a known issue.

Search extension MUST declare one of:

```yaml
boundary_mode: "none" | "adjacent_decode" | "adjacent_window_index"
boundary_window_bytes: u64
```

Recommended extension behavior:

```yaml
boundary_mode: "adjacent_window_index"
boundary_window_bytes: 4096
```

Then:

```text
- single-chunk matches can be complete=true
- boundary matches within boundary_window_bytes can be complete=true
- longer boundary-spanning matches require fallback scan or complete=false
```

### 29.9 Query planner

Search planners SHOULD minimize decompression by ordering operations as:

```text
1. parse query into required token/ngram keys
2. look up Term Dictionary entries
3. sort required posting lists by increasing granule_frequency
4. intersect rare posting lists first using skip data
5. stop if candidate count or decoded byte estimate exceeds limits
6. map surviving Search Granules to chunk ranges
7. merge overlapping chunk reads
8. decode and verify exact matches against original bytes
```

For phrase or substring search, the planner SHOULD use the rarest required n-gram first, then verify exact byte ranges by partial decode.

If a required key is missing from an index with `complete=true`, the planner MAY return no matches without decoding original chunks.

If a required key is missing from an index with `complete=false`, the planner MUST either run a documented fallback scan or report that the index cannot prove completeness.

### 29.10 High document frequency terms

Search planners MUST protect against huge candidate sets.

Recommended metadata:

```yaml
max_candidate_granules_default: 10000
max_decoded_bytes_default: 268435456
high_df_per_million: 200000
high_df_terms: supported
```

If a term's `granule_frequency * 1_000_000 / granule_count` is greater than or equal to `high_df_per_million`, the planner SHOULD treat it as a high document-frequency term and avoid using it as the first intersection driver.

If candidate granules exceed `max_candidate_granules_default`, the planner SHOULD require a narrower query, an explicit higher limit, or a fallback scan mode.

CLI SHOULD expose:

```bash
qzt search data.qzt "error" --max-candidates 10000 --max-decoded-bytes 256MiB
```

### 29.11 Search performance reporting

Search extension implementations SHOULD report these metrics for benchmark and debug runs:

```yaml
query: string
index_kind: "token" | "ngram" | "hybrid"
posting_granularity: string
index_size_bytes: u64
source_size_bytes: u64
index_size_ratio: float
term_lookups: u64
posting_bytes_read: u64
candidate_granules: u64
candidate_chunks: u64
decoded_bytes: u64
verified_matches: u64
query_time_ms: float
```

A search index SHOULD NOT be described as high performance without reporting at least:

```text
- candidate_granules
- candidate_chunks
- decoded_bytes
- query_time_ms
- index_size_ratio
```

### 29.12 Search implementation cut

A high-performance Search Extension implementation SHOULD be built after Core conformance and SHOULD include:

```text
- Granule Table builder
- Term Dictionary builder
- sorted delta-varint posting lists
- skip data for long posting lists
- query planner with rarest-first intersection
- exact verification against original bytes
- benchmark reporting
```

---

## 30. Extension: Sidecar indexes

A sidecar index MAY be used when search/vector/FM-index data is too large or should be rebuilt independently.

Sidecars are the recommended deployment model for high-performance search over large containers:

```text
.qzt = cold, immutable, verifiable evidence container
.qzi = hot, rebuildable, memory-mappable search index
```

Keeping large search structures in a sidecar lets implementations rebuild, tune, shard, or cache indexes without rewriting the source evidence container.

Suggested names:

```text
data.qzt.qzi     general QZT side index
data.qzt.fm      FM-index sidecar
data.qzt.vec     vector sidecar
```

A sidecar MUST include:

```yaml
schema: "qzt.sidecar.v1"
source_container_id: bstr16
source_format_version: [0, 1]
source_original_checksum:
  algorithm: "blake3"
  value: bstr32
source_qzt_footer_checksum:
  algorithm: "blake3"
  value: bstr32
index_type: string
created_at_unix_ms: u64
index_manifest:
  schema: string
  kind: string
  posting_granularity: string
  index_size_bytes: u64
  source_size_bytes: u64
```

Readers MUST reject a sidecar if `source_container_id` or source checksum does not match.

A search sidecar SHOULD store the Search Manifest defined in Section 29 and MAY store the binary sections directly in the sidecar file rather than inside the `.qzt` Index Root.

Search sidecars SHOULD be designed for memory-mapped lookup:

```text
- fixed or bounded-size sidecar header
- deterministic CBOR manifest near the start
- sorted term dictionary section
- posting sections grouped for locality
- skip data close to posting lists
```

Sidecar indexes MUST remain derived data. If a sidecar is missing or rejected, Core QZT read/export/verify behavior MUST still work.

---

## 31. Extension: Optimizer metadata

Optimizer metadata MAY record how encoder parameters were chosen.

It MUST NOT be required for decoding.

Index Root descriptor:

```yaml
type: "optimizer_metadata"
required: false
codec: "cbor"
```

Allowed example:

```yaml
optimizer:
  kind: "quantum-inspired" | "heuristic" | "grid-search"
  objective:
    compressed_size_weight: 0.5
    query_latency_weight: 0.3
    index_size_weight: 0.2
  selected:
    target_chunk_size: 1048576
    zstd_level: 12
```

Decoder MUST ignore optimizer metadata.

---

## 32. Compatibility with zstd

QZT v0.1 Core is not a `.zst` stream.

This MUST be represented in metadata:

```yaml
compatibility:
  qzt_is_zst_stream: false
  chunks_are_independent_zstd_frames: true
```

A standard zstd decoder is not expected to decode an entire `.qzt` file.

However, each compressed chunk is an independent zstd frame, and can be decoded by a zstd decoder if its physical offset, compressed size, and dictionary are known.

Future versions MAY define a zstd-skippable-compatible profile.

---

## 33. Security and resource limits

Readers MUST validate:

```text
- all offsets are inside file bounds
- all sizes are reasonable and do not overflow u64
- physical chunk ranges do not overlap invalidly
- metadata, index, chunk, footer payload, and footer trailer ranges do not overlap invalidly
- uncompressed chunk size <= configured max
- dictionary size <= configured max
- index block size <= configured max
- footer/index blocks do not point to themselves cyclically
- required dictionaries exist
- decompression does not exceed declared uncompressed_size
- unknown flag bits are rejected
- deterministic CBOR requirements are enforced before trusting CBOR fields
```

Recommended default limits:

```yaml
max_uncompressed_chunk_size: 64MiB
max_dictionary_size: 16MiB
max_index_block_size: configurable
max_search_results: 100000
max_preview_bytes: 1MiB
```

Implementations MUST protect against decompression bombs.

---

## 34. Conformance levels

### 34.1 QZT v0.1 Reader Core

A Reader Core implementation MUST support:

```text
- open
- info
- export
- read_range
- read_line_raw
- verify quick/normal/deep
- QZT deterministic CBOR validation
- Metadata/Header/Footer/Index Root consistency checks
- zstd chunks without dictionary
- zstd chunks with embedded dictionary if dictionary block is present
- sparse line index via Chunk Table
```

### 34.2 QZT v0.1 Writer Core

A Writer Core implementation MUST support:

```text
- pack UTF-8 input
- independent zstd frames
- valid Header
- valid Metadata
- valid Chunk Table
- valid Index Root
- valid Footer Payload
- valid Footer Trailer
- QZT deterministic CBOR encoding
- BLAKE3 compressed/uncompressed chunk checksums
- UTF-8 safe chunk boundaries
- sparse line index via Chunk Table
```

Writer Core MAY omit embedded dictionary output. If it emits dictionary-compressed chunks, it MUST emit a valid Dictionary Block.

### 34.3 QZT v0.1 Search Extension

Search extension implementation MAY support:

```text
- token index
- ngram index
- Search Granule candidate search
- Term Dictionary lookup
- sorted posting list intersection
- skip data
- rarest-first query planning
- exact verification against original bytes
- boundary mode declaration
- raw/normalized index separation
- search performance reporting
```

Search extension is not required for Core conformance.

---

## 35. Test suite

A QZT v0.1 Reader Core or Writer Core implementation SHOULD pass all Core conformance tests that apply to its role.

### 35.1 Core conformance tests

```text
1. empty file
2. one line without newline
3. one line with newline
4. LF multi-line
5. CRLF multi-line
6. mixed LF/CRLF
7. lone CR treated as ordinary data
8. Japanese UTF-8
9. emoji UTF-8
10. invalid UTF-8 rejected by writer
11. very long single line exceeding target_chunk_size
12. very long single line exceeding max_chunk_size
13. read_line for a line spanning multiple chunks
14. chunk boundary at Japanese multibyte character rejected or avoided
15. chunk boundary between CR and LF rejected or avoided
16. small chunk size
17. no dictionary
18. embedded dictionary fixture can be read
19. missing dictionary rejected
20. duplicate dictionary_id rejected
21. dictionary checksum mismatch rejected
22. corrupted Header magic
23. non-zero Header reserved bytes rejected
24. non-zero header_flags rejected
25. unsupported version rejected
26. Header/Footer/Payload/Metadata/Index Root version mismatch rejected
27. corrupted Footer Trailer
28. Footer Payload checksum mismatch
29. final_file_size mismatch
30. Header/Footer container_id mismatch
31. Header/Footer metadata offset mismatch
32. Metadata checksum mismatch
33. Metadata non-canonical CBOR rejected
34. Metadata duplicate CBOR key rejected
35. Metadata/Header/Footer container_id mismatch
36. Metadata/Index Root original_size mismatch
37. Metadata/Index Root original_checksum mismatch
38. Metadata/Index Root line_count mismatch
39. corrupted Index Root checksum
40. Index Root non-canonical CBOR rejected
41. block descriptor overlap rejected
42. unknown optional block ignored
43. unknown required block rejected
44. non-zero block descriptor flags rejected
45. corrupted Chunk Table checksum
46. Chunk Table invalid chunk_id sequence
47. Chunk Table logical gap rejected
48. Chunk Table physical range out of bounds rejected
49. Chunk Table physical overlap rejected
50. Chunk Table first_line continuity invalid
51. Chunk Table sum(line_count) mismatch
52. unknown chunk flag rejected
53. starts_with_line_continuation flag mismatch detected by deep verify
54. corrupted compressed chunk bytes
55. corrupted uncompressed checksum after decode
56. read_range spanning one chunk
57. read_range spanning multiple chunks
58. read_range offset + length overflow rejected
59. read_text_range invalid UTF-8 boundary rejected
60. read_line first line
61. read_line last line without trailing newline
62. read_line last line with trailing newline
63. read_line out of range
64. Dense Line Index final line without newline, if Dense Line Index is present
65. Dense Line Index line_start_offsets count mismatch, if Dense Line Index is present
66. export equality
67. quick verify succeeds without decompressing chunks
68. normal verify detects compressed chunk checksum mismatch
69. deep verify detects invalid uncompressed line_count
70. Chunk Table block size not equal to chunk_count * 128 rejected
71. Chunk Table chunk_count mismatch rejected
72. zero-length chunk rejected
73. sum(uncompressed_size) original_size mismatch rejected
74. deep verify detects invalid newline_mode
75. zstd frame output exceeding declared uncompressed_size rejected
76. invalid index_hint_offset is ignored when Footer Trailer path is valid
77. normal verify detects container_checksum mismatch when present
```

### 35.2 Extension conformance tests

Implementations that claim an extension SHOULD pass that extension's tests:

```text
1. Document Index ranges within original_size
2. Document Index chunk_start/chunk_end consistency
3. token index candidate false positive verified against original text
4. ngram index boundary_mode declaration required
5. sidecar wrong source_container_id rejected
6. sidecar wrong source_original_checksum rejected
7. optimizer metadata ignored by decoder
8. Search Granule table ranges are inside original_size
9. Search Granule chunk_start/chunk_end cover every granule range
10. Term Dictionary entries are sorted and exact key comparison is enforced
11. posting lists are sorted by increasing granule_id
12. delta-varint posting list round-trips large granule IDs
13. skip data allows intersection without decoding an entire high-frequency posting list
14. rarest required posting list is selected first by the query planner
15. high document-frequency term does not drive first intersection by default
16. missing key in complete=true index returns no matches without chunk decode
17. missing key in complete=false index triggers documented fallback or incompleteness result
18. byte_window overlap preserves declared complete boundary matches
19. phrase or substring results are verified against original bytes
20. search benchmark report includes candidate_granules, candidate_chunks, decoded_bytes, query_time_ms, and index_size_ratio
```

---

## 36. Reference implementation roadmap

Recommended implementation order:

```text
Stage 0:
  - QZT deterministic CBOR encoder/decoder profile
  - fixed Header
  - fixed Footer Trailer
  - Footer Payload
  - Metadata
  - Index Root skeleton
  - Chunk Table skeleton
  - Header/Footer/Metadata/Index Root consistency checks

Stage 1:
  - independent zstd chunks
  - pack/export exact equality
  - compressed/uncompressed chunk checksums
  - quick/normal/deep verify

Stage 2:
  - sparse line index
  - starts_with_line_continuation chunk flag
  - line-spanning chunk reads
  - qzt line
  - qzt range --bytes
  - qzt range --lines
  - head/tail convenience commands

Stage 3:
  - embedded dictionary reader support
  - optional embedded dictionary writer support
  - Dense Line Index
  - Document Index
  - memory profile

Stage 4:
  - Token Index
  - Ngram Index
  - qzt search
  - boundary match handling

Stage 5:
  - dictionary training
  - sidecar FM-index
  - sidecar vector index
  - optimizer metadata
```

Reader Core conformance is complete after Stage 3 embedded dictionary reader support.
Writer Core conformance is complete after Stage 2 if the writer does not emit dictionaries, or after Stage 3 if it emits dictionary-compressed chunks.

### 36.1 Reference implementation cut lines

To make QZT implementable, a reference implementation SHOULD use explicit cut lines.

#### Cut 0: format foundation

Build and test these before writing compressed data:

```text
- fixed binary Header and Footer Trailer encode/decode
- QZT deterministic CBOR encode/decode and rejection tests
- BLAKE3 helpers
- physical range validation helpers
- checked u64 arithmetic helpers
- QZT error type
- zstd single-frame encode/decode wrapper with output limits
```

Done criteria:

```text
- invalid magic/version/flags are rejected
- non-canonical CBOR is rejected
- range overflow is rejected
- empty byte strings can be checksummed deterministically
```

#### Cut 1: no-dictionary pack/export

Build the first writer with this restricted scope:

```text
- UTF-8 input only
- no dictionary output
- no Dense Line Index
- no Document Index
- no Search Index
- one zstd frame per chunk
- valid Metadata, Chunk Table, Index Root, Footer Payload, Footer Trailer
- Header patch at finish
```

The reference implementation task plan splits this cut into chunk planning and zstd/file finalization so UTF-8 boundaries, CRLF handling, and sparse line semantics can be tested before compressed I/O is added.

Done criteria:

```text
- export(pack(input)) == input for empty, ASCII, CRLF, Japanese, emoji, and long-line fixtures
- quick verify passes without decompression
- normal verify detects compressed chunk corruption
- deep verify detects uncompressed checksum, line_count, and newline_mode corruption
```

#### Cut 2: random access reader

Build reader operations on top of the validated Chunk Table:

```text
- read_range
- read_text_range
- read_line_raw
- read_line_text
- line-spanning chunk reads
- starts_with_line_continuation handling
```

Done criteria:

```text
- zero-length reads return empty bytes
- byte ranges spanning chunks match slices from original input
- line reads match original lines with and without final newline
- long single lines split across chunks can be read exactly
```

#### Cut 3: Reader Core completion

Add the remaining Reader Core obligations:

```text
- embedded Dictionary Block parsing
- dictionary checksum validation
- dictionary-assisted zstd decode
- unknown optional block handling
- unknown required block rejection
- resource limit enforcement
```

Done criteria:

```text
- dictionary-compressed fixture can be exported exactly
- missing, duplicated, or corrupted dictionaries are rejected
- unknown optional blocks do not break open/info/export
- unknown required blocks fail with UnknownRequiredBlock
```

#### Cut 4: optional Core-defined indexes

Only after Cut 3 should the reference implementation add:

```text
- Dense Line Index writer
- Dense Line Index reader fast path
- Dense Line Index deep verification
```

Dense Line Index MUST remain an acceleration structure. Correctness MUST still come from decoded original bytes and Chunk Table line-start semantics.

#### Cut 5: extensions

Document Index, Search Index, Sidecar indexes, and Optimizer metadata SHOULD NOT be implemented until Core conformance tests pass.

Extension implementations SHOULD have separate conformance fixtures and MUST NOT be required for QZT v0.1 Core conformance.

For high-performance search, Cut 5 SHOULD be split into:

```text
Cut 5a:
  - Document Index, if document granularity is needed
  - Search Granule Table
  - Token Index builder
  - Term Dictionary lookup
  - sorted delta-varint postings
  - exact verification by partial decode

Cut 5b:
  - N-gram Index builder
  - byte_window granularity
  - boundary window handling
  - phrase/substring query verification

Cut 5c:
  - skip data
  - rarest-first query planner
  - high document-frequency term handling
  - decoded-byte and candidate limits
  - benchmark reporting

Cut 5d:
  - search sidecar writer/reader
  - memory-mapped dictionary/posting lookup
  - sidecar rebuild command
```

Done criteria for high-performance search:

```text
- queries report candidate_granules, candidate_chunks, decoded_bytes, and query_time_ms
- common-term queries are capped or require explicit fallback mode
- rare-term queries decode only chunks overlapping candidate granules
- phrase and substring results are verified against original bytes
- sidecar rejection never breaks Core read/export/verify
```

---

## 37. Final v0.1 Core summary

QZT v0.1 Core is:

```text
Fixed Header, 128 bytes
+ QZT deterministic CBOR profile
+ canonical CBOR Metadata
+ independent zstd frames
+ Chunk Table
+ sparse Line Index via Chunk Table
+ Index Root
+ Footer Payload
+ Footer Trailer, 64 bytes
```

Its required guarantees are:

```text
- exact export
- partial byte-range read
- line read
- line-spanning chunk read
- UTF-8 safe chunking
- CRLF-safe chunk boundaries
- compressed checksum
- uncompressed checksum
- metadata/index/header/footer consistency checks
- quick/normal/deep verify
- immutable after finish
```

Its product identity is:

```text
QZT is not better compression.
QZT is better evidence access.
```

For Memory Pager:

```text
QZT is the Cold Evidence Container.
Memory Pager stores summaries and memory hierarchy.
QZT stores exact source evidence and restores only the requested ranges.
```
