# Phase17: Streaming Writer

[English](Phase17.md)

## 目的

RAM より大きいコンテナを producer が作れるように、実際の `QztFileWriter<W: Write + Seek>` を
実装します。現状の writer は fully buffered input に対する one-shot `pack_bytes` であり、
`QztWriter` は `#[doc(hidden)]` placeholder です。

Streaming writer は input を incremental に受け取り、既存 chunker で chunk boundary に分割し、
各 frame を compress して書き出します。in-memory には Chunk Table（128 bytes per chunk）だけを
蓄積し、`finish()` で metadata、optional blocks、Chunk Table、Index Root、Footer Payload、
Trailer を書き、最後に Header を seek-back で patch します。

この Phase は同じ input と options に対して `pack_bytes` と byte-identical な output を生成しなければ
なりません。したがって format を変えたり、`export(pack(input)) == input` を壊したりできません。

## Minimum MVP

```text
- QztFileWriter::new(writer, options)
- push(&[u8]) は call 間で buffer し、既存 chunker で boundary split し、compress して frame を書き、ChunkEntry を記録する
- finish() は metadata, Chunk Table, Index Root, Footer Payload, Trailer を書き、seek-back で Header を patch する
```

## Goal MVP

```text
- streaming writer output は同じ input/options の pack_bytes と byte-identical（differential golden test）
- writer peak memory は input size ではなく max chunk size + Chunk Table に bound される
- CLI pack は file または stdin を全読みせず writer へ stream する
- finish() は single-shot: error 後または finish 後の writer は poisoned で、valid と主張できる container を emit できない
```

## Spec refs

```text
- Section 22 Immutability（finish seals the container）
- Writer API and finish semantics の section
- Footer Payload fixed-point final_file_size convergence の section
```

## Conformance Tests Covered

```text
- streaming writer output は fixtures/options 全体で pack_bytes output と byte-for-byte で一致する
- streaming writer round-trip: export(stream_pack(input)) == input
- 多数の push() call に分割した partial input は one push と同じ container を作る
- finish() は二度呼べない。poisoned writer は valid-looking container を出さない
- writer は one-shot writer と同じ UTF-8 validity を chunk boundaries で強制する
```

## TDD Plan

失敗する test を先に書きます。

```text
- stream_pack(input) は empty, single-line, multi-line, CRLF, mixed, UTF-8 fixtures で pack_bytes(input) と byte-equal
- input を 1-byte, prime-sized, chunk-sized increments で push しても同一 container になる
- streamed container の export は original input と一致する
- writer peak allocation は allocation probe で max chunk size + Chunk Table に bound される
- mid-stream の invalid UTF-8 は one-shot writer と同じ error variant で拒否される
- finish() を二度呼ぶと specific error を返し、second container を emit しない
```

## Implementation Tasks

```text
1. QztFileWriter<W: Write + Seek> と Header patch 用 WriteAt/seek-back helper を定義する
2. push() calls を跨ぐ boundary buffer を持ち、chunker boundary logic を再利用する
3. finalized chunk ごとに compress し、frame を書き、両 BLAKE3 checksum 付き ChunkEntry を追加する
4. finish() で metadata / optional blocks / Chunk Table / Index Root / Footer Payload / Trailer を書く
5. 既存の final_file_size fixed-point footer convergence を再利用する
6. seek back して Header metadata offset/size を patch する
7. finish() 後または fatal error 後に writer を poison する
8. pack_bytes との differential golden test を追加する
9. pack CLI で stdin/file を writer へ stream する
```

## Rust Notes

Chunk Table は 128 bytes per chunk で memory に増えます。この bound を文書化し、極端な chunk count
での Chunk Table spill は明示的に defer します。chunker と fixed-point footer routine は再実装せず
既存 logic を再利用します。Header patch には `Seek` が必要なので、`Write + Seek` bound を維持し、
sink が seekable でない場合は明確に fail します。`finish()` は Section 22 の immutability boundary
として扱い、sealed 後は追加 write できません。

## Review Gates

この Phase を done にする前に code review を完了しなければなりません。

この Phase を done にする前に architecture review を完了しなければなりません。

どちらかの review で spec ambiguity や library constraint が見つかった場合は、続行前に
spec とこの phase plan を更新します。

## Self-Review Checklist

```text
- streaming output は全 fixture/options で pack_bytes と byte-identical か
- peak memory は input size と独立し chunk size + Chunk Table に bound されているか
- push fragmentation が resulting container を変えないか
- finish() は二度実行できない hard immutability boundary か
- UTF-8 / CRLF boundary rules は one-shot writer と同一か
- format byte change を避けたか
```

## Done Criteria

```text
- QztFileWriter push/finish が実装済み
- byte-identical differential golden tests が通る
- fragmentation-invariance tests が通る
- peak-memory bound test が通る
- double-finish poisoning test が通る
- pack CLI が full-input buffering なしで stream する
- code review findings が修正済み
- architecture review findings が修正済み
- status.md が更新済み
```

## 状態

Complete。

完了日: 2026-06-08

実装範囲:

```text
- readable / writable / seekable output 上の QztFileWriter push/finish を追加。
- core/no-dense containers の streaming pack CLI は QztFileWriter 経由で書き、invalid UTF-8 時に output を残さない。
- covered core fixtures では pack_bytes と byte-identical。
```

検証:

```text
- cargo test --test phase17_streaming_writer
- make check
```

Review notes:

```text
- Self-review pass 1 completed: checksum hashing のために compressed chunks を保持していた実装を削除し、bounded 64KiB prefix readback に修正。
- Self-review pass 2 completed: CLI temporary output は finish が final prefix を hash できるよう read/write OpenOptions を使う。
- Code review completed: finish は single-shot、poisoned writer は fail closed、invalid UTF-8 は final output file を残さない。
- Architecture review completed: writer memory は full source / compressed payload ではなく pending chunk、chunk table metadata、fixed hash buffer に bounded。
```

依存: Phase15（shared WriteAt/ReadAt direction と round-trip differential tests の decode core）。
file-backed reader で始めた large-data I/O model を完成させます。H-5 placeholder writer を置換します。
