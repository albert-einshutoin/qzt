# QZT v0.1 Core Specification 日本語版

[English normative specification](QZT_v0.1_Core_Spec.md)

状態: Draft Complete  
仕様 version: 0.1.0  
日付: 2026-06-07  
拡張子: `.qzt`  
名称: **QZT: Queryable Zstd Text Container**

## 注意

この日本語版は、公開リポジトリで仕様の意図を追いやすくするための companion document です。細かい MUST/SHOULD/MAY、byte layout、field 名、error code、conformance 判定の正本は英語版 [QZT_v0.1_Core_Spec.md](QZT_v0.1_Core_Spec.md) です。

## 0. 概要

QZT は大きな UTF-8 text data を保存する binary container format です。元テキストを独立した zstd frame の chunk に分け、Metadata、Chunk Table、Index Root、Footer Payload、Footer Trailer を追加します。

QZT が実現すること:

```text
- container 全体を export せずに構造と integrity を verify する
- export(pack(input)) == input を満たす
- 必要な byte range だけを読む
- logical line number で読む
- summary / memory record から original source text へ戻る evidence pointer を持つ
- optional な document/search side index を Core format から分離する
```

QZT は zstd の置き換えではありません。zstd を compression engine として使い、その上に seekable / verifiable / evidence-addressable な text container layer を定義します。

## 1. Scope

### Core に含むもの

```text
- exact export
- independent zstd chunks
- 128-byte fixed Header
- deterministic CBOR Metadata
- Chunk Table
- Chunk Table による sparse line index
- 64-byte Footer Trailer
- Footer Payload
- Index Root block directory
- byte range read
- line read
- quick / normal / deep verification
- UTF-8 chunk boundary safety
- compressed / uncompressed chunk checksums
- finish 後 immutable container
```

### Core に含めないもの

```text
- semantic search
- vector embeddings / vector DB behavior
- LLM memory ranking
- summarization
- mutable database updates
- arbitrary binary archive semantics
- ZIP compatibility
- .zst stream compatibility
- mandatory token/ngram search
- mandatory document index
```

これらは optional extension として定義できます。

## 2. Product boundary

QZT は次のように位置づけます。

```text
seekable + verifiable + evidence-addressable text container
```

主張してはいけないこと:

```text
- zstd より良い compression algorithm
- 任意 text を decompression なしで表示すること
- external layer なしの semantic search
- FM-index / vector DB / Memory Pager の置き換え
```

正しい説明は「compressed text 全体を展開せず、関係する compressed chunk に絞って必要部分だけ decode する」です。

## 3. Memory Pager との関係

Memory Pager は semantic search、hierarchy、ranking、LLM context assembly を扱います。QZT は original evidence を lossless に保存し、partial restore と verification を担当します。

依存方向:

```text
Memory Pager uses QZT.
QZT does not depend on Memory Pager.
```

## 4. File overview

QZT file は概念的に以下の構造です。

```text
[Header: 128 bytes]
[zstd chunk 0]
[zstd chunk 1]
...
[Metadata CBOR]
[optional extension blocks]
[Chunk Table]
[Index Root CBOR]
[Footer Payload CBOR]
[Footer Trailer: 64 bytes]
```

物理 range は half-open `[offset, offset + size)` です。overflow や file bounds を必ず検査します。

## 5. Deterministic CBOR

Metadata、Footer Payload、Index Root、Document Index などの CBOR block は deterministic CBOR profile に従います。

重要な制約:

```text
- shortest integer encoding
- definite length only
- map keys sorted by encoded byte order
- duplicate keys forbidden
- tags / floats forbidden
- unknown fields は schema ごとに明示処理
```

## 6. Header / Footer

Header は固定 128 bytes です。`index_hint_offset` は fast path hint であり、authoritative ではありません。Reader は Footer Trailer と Footer Payload を検証した上で Index Root を信頼します。

Footer Trailer は固定 64 bytes です。Footer Payload の offset、size、checksum を持ちます。

Footer Payload は final file size、Metadata ref、Index Root ref、container checksum などを持ちます。

## 7. Metadata

Metadata は source identity、original size/checksum、newline mode、line count、compression、chunking、index presence、compatibility を記録します。

Writer は実際に使った `zstd_level`、`target_chunk_size`、`max_chunk_size` を保存しなければなりません。

## 8. UTF-8 と line semantics

QZT Core は UTF-8 text container です。chunk boundary は UTF-8 code point の途中で切ってはいけません。CRLF の `\r\n` の間で切ってはいけません。

Line numbering は API 内部では 0-based、CLI の通常表示は 1-based です。line output は original bytes を返し、newline が存在する場合は newline も含みます。

## 9. Chunks and Chunk Table

各 chunk は独立した zstd frame です。Chunk Table は 128-byte fixed record の配列です。

Chunk Entry は以下を表します。

```text
- chunk_id
- physical_offset / compressed_size
- logical_offset / uncompressed_size
- first_line / line_count
- dictionary_id
- flags
- compressed checksum
- uncompressed checksum
```

Chunk Table は chunk_id、physical/logical order、overlap、bounds、checksums、line metadata の整合性を満たす必要があります。

## 10. Verification levels

```text
quick:
  fixed structures、CBOR block refs、checksums、Chunk Table shape を確認する。
  compressed chunk は decode しない。

normal:
  quick に加えて compressed chunk checksum や container checksum を確認する。

deep:
  全 chunk を decode し、uncompressed checksum、UTF-8、line_count、newline_mode、
  continuation flag、optional index consistency を確認する。
```

## 11. CLI

Core CLI:

```text
qzt pack input.txt -o output.qzt
qzt info data.qzt
qzt export data.qzt -o restored.txt
qzt range data.qzt --bytes 1048576:2097152
qzt range data.qzt --lines 1000:1200
qzt line data.qzt 1000
qzt verify data.qzt --quick|--normal|--deep
```

Search / sidecar CLI:

```text
qzt search data.qzt "error"
qzt sidecar-rebuild data.qzt -o data.qzt.qzi
qzt search data.qzt "error" --sidecar data.qzt.qzi
```

## 12. Profiles

Profiles は writer defaults と optional index behavior を調整します。Core conformance を弱めるものではありません。

```text
minimal: optional block をできるだけ持たない
core: default Core behavior
log: log-oriented defaults
archive: archive-oriented defaults
memory: Dense Line Index / Document Index / search sidecar と相性の良い profile
```

## 13. Optional extensions

Core 外の拡張:

```text
- Dense Line Index
- Document Index
- Token Index
- N-gram Index
- Search Granules
- QZI sidecar
- Optimizer metadata
```

Search extension は candidate search だけで match を確定してはいけません。candidate chunk / granule を decode し、original bytes で verified hit にする必要があります。

## 14. Security and resource limits

Reader は untrusted input を扱う前提です。

```text
- offset + size overflow を拒否
- allocation limit を確認
- decompression bomb を防ぐ
- malformed CBOR を拒否
- unknown required block を拒否
- unknown optional block は安全に無視
- corrupt file で panic しない
```

## 15. Conformance levels

```text
Reader Core:
  valid Core container を open/export/range/line/verify できる。

Writer Core:
  valid Core container を生成し、export(pack(input)) == input を満たす。

Search Extension:
  raw token/ngram search、candidate planning、verified original-byte hits、
  benchmark metrics、sidecar validation を満たす。
```

## 16. Test suite

Core conformance tests は fixed structures、CBOR、Metadata、Footer、Chunk Table、UTF-8、line access、verification、resource limits、CLI を対象にします。

Extension tests は Dense Line Index、Document Index、Search Granules、Token Index、N-gram Index、planner、sidecar を対象にします。

## 17. Reference implementation roadmap

実装 cut:

```text
Cut 0: format foundation
Cut 1: no-dictionary pack/export
Cut 2: random access reader
Cut 3: Reader Core completion
Cut 4: optional Core-defined indexes
Cut 5: extensions
```

この repository では `tasks/Phase0.md` から `tasks/Phase13.md` として具体化しています。

## 18. v0.1 Core summary

QZT v0.1 Core の要点:

```text
QZT is not better compression.
QZT is better evidence access.
```

QZT は text evidence を immutable、seekable、verifiable に保存し、後続の memory/search system が正確な source bytes に戻れるようにする container format です。
