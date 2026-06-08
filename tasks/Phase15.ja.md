# Phase15: File-Backed Seeking Reader

[English](Phase15.md)

## 目的

「大きなコンテナに対して、全体をメモリに読み込まずにアクセスする」という製品の中核価値に
実装を合わせます。現状の `QztReader` はコンテナ全体を `Vec<u8>` に保持しており、
「大きなテキストを全展開なしで扱う」という主張と矛盾しています。

この Phase では `ReadAt` 抽象と `QztFileReader<R: Read + Seek>` を導入します。
`QztFileReader` は fixed trailer、footer payload、header、metadata、index root、
chunk table だけを読んで open し、chunk-data 領域は open 時に読みません。range / line read
は要求に重なる chunk だけへ seek し、必要な chunk だけ decode します。

この Phase はコンテナフォーマットのバイトや verification semantics を変更してはいけません。
file reader と in-memory reader は、すべての fixture で同じ結果を返さなければなりません。

## Minimum MVP

```text
- ReadAt trait: read_exact_at(offset, buf)、slice impl、File impl
- QztFileReader::open は bounded prefix/suffix と index region だけを読み、chunk-data region を読まない
- read_range は start chunk へ seek し、重なる chunk だけ decode する
- in-memory QztReader は維持し、shared decode/verify core を抽出して logic duplication を避ける
```

## Goal MVP

```text
- file reader の read_line_raw は Chunk Table と optional Dense Line Index を使い、必要 chunk だけ seek する
- export_to は file から chunk-by-chunk で bounded buffer に stream する
- range/line read の peak resident memory は max chunk size + index region に bound され、file size に依存しない
- CLI range / line / export は file-path input に file reader を使う
```

## Spec refs

```text
- Section 9.1 reader open procedure
- Section 12 range reads and UTF-8 boundary handling
- Section 13 line semantics
- tasks/README.md Rust Style: ReadAt / WriteAt behavior の trait
```

## Conformance Tests Covered

```text
- file reader open は bounded byte count だけを読み、chunk-data region に触れない
- file reader と in-memory reader は全 fixture の info/export/range/line で一致する
- file reader range read は要求 range に重なる chunk だけ decode する
- corrupt/out-of-bounds physical offsets は seeking path で panic せず拒否される
- resource limits は decode 前に file path でも強制される
```

## TDD Plan

失敗する test を先に書きます。

```text
- counting ReadAt が open() の読み取り範囲を記録し、trailer/footer/header/metadata/index root/chunk table bytes だけを読むことを確認する
- open() は chunk-data region 内の byte を読まない
- file reader の read_range が全 fixture で in-memory reader の read_range と一致する（differential test）
- N chunk に跨る read_range は、その N compressed frames だけを読み、それ以上読まない
- file reader の read_line_raw が spanning lines を含め in-memory reader と一致する
- file reader の export_all が全 fixture で original input と一致する
- corrupt chunk physical_offset を持つ request は panic せず specific error を返す
- ResourceLimits の chunk-size / index-size cap が file path でも強制される
```

## Implementation Tasks

```text
1. ReadAt trait を定義し、&[u8] と std::fs::File に impl する（positioned read、seek+read fallback）
2. shared decode-and-verify chunk core を抽出し、両 reader が同じ logic を呼ぶようにする
3. bounded prefix/suffix と index region だけを読む QztFileReader::open を実装する
4. Chunk Table binary search と per-chunk seek/decode を使った file-backed read_range を実装する
5. Chunk Table と optional Dense Line Index fast path を使った file-backed read_line_raw を実装する
6. file-backed export_to を chunk-by-chunk streaming で実装する
7. 全 fixture を両 reader で走らせる differential test harness を追加する
8. CLI file-path input の range/line/export を file reader に接続する
9. operation ごとの peak-memory bound を文書化する
```

## Rust Notes

`ReadAt` は testing に効く範囲で minimal かつ object-safe に保ちます。利用可能な環境では
positioned reads を優先しますが、この Phase では Send + Sync concurrency を scope に入れません。
Phase21 が public evidence API の後に concurrent verified-read guarantee を所有します。この Phase は
まず single-reader seeking path を確立します。

decode-and-verify core は reader 間で共有し、checksum と boundary logic が duplicated されたり
drift したりしないようにします。container data 由来の file offset はすべて checked arithmetic で
検証します。

## Review Gates

この Phase を done にする前に code review を完了しなければなりません。

この Phase を done にする前に architecture review を完了しなければなりません。

どちらかの review で spec ambiguity や library constraint が見つかった場合は、続行前に
spec とこの phase plan を更新します。

## Self-Review Checklist

```text
- file reader は全 fixture で in-memory reader と byte-identical な結果を返すか
- open() が chunk-data region を読まないことを証明できるか
- peak memory は file size ではなく chunk size + index に bound されているか
- container data 由来の全 file offset が checked arithmetic で検証されているか
- decode/verify core が duplicate ではなく shared か
- in-memory reader public API を変えていないか
```

## Done Criteria

```text
- ReadAt trait と File/slice impl が存在する
- QztFileReader open/info/export/range/line が実装済み
- differential tests が全 fixture で通る
- open-reads-bounded-prefix test が通る
- peak-memory bound test が通る
- CLI が file-path input に file reader を使う
- code review findings が修正済み
- architecture review findings が修正済み
- status.md が更新済み
```

## 状態

Pending。

依存: Phase14（より大きい differential test matrix を CI で実行できるようにするため）。
これは Product Completeness で最も重要な Phase です。製品の主張と実装の差を閉じます。
status follow-up table の "M-4 file-path seeking reader" に対応します。
