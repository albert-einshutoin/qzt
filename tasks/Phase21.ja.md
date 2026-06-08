# Phase21: Verified Evidence Retrieval and Memory Pager Integration

[English](Phase21.md)

## 目的

製品を定義する operation、つまり pointer による verified evidence retrieval を届け、Memory Pager
integration story を end to end で証明します。Spec Section 3 は `container_id`, `doc_id`,
`byte_range`, `line_range`, `checksum` を持つ `evidence_ref` を定義しています。Memory Pager は
その range を restore し、stored checksum と照合します。

現状 range reads は存在しますが、「restore して expected checksum と verify する」単一 operation は
ありません。file-backed `doc_id` resolution は Phase15 ではなくこの Phase で public evidence API と
一緒に導入します。また Section 3 workflow を証明する example / integration test もありません。この
Phase はその gap を閉じ、QZT が AI memory system の Cold Evidence Container として demonstrably usable
であることを示します。

この Phase は container format bytes を変更してはいけません。Verified retrieval は既存構造上の
read-side capability です。

## Minimum MVP

```text
- read_range_verified: byte range を restore し expected BLAKE3 checksum と verify し、mismatch では specific error を返す
- read_document_verified: doc_id range を resolve し expected BLAKE3 checksum と verify する
- real container に対する Section 3 evidence_ref workflow を示す examples/ program
```

## Goal MVP

```text
- end-to-end integration test: container を作り、evidence_ref-like pointer を emit し、path + container_id で reopen し、byte range と document を restore/verify する
- evidence-retrieval API と Memory Pager integration pattern を README または usage guide に文書化する
- 1 open container からの concurrent verified reads を test する（Phase15 の file-backed reader path を土台にする）
- tampered container byte は該当 verified read を specific error で fail させ、wrong bytes を silent に返さない
```

## Spec refs

```text
- Section 2 product boundary
- Section 3 relationship to Memory Pager and the evidence_ref shape
- Section 12 range reads
- Section 13 line semantics
- Section 28 Document Index
```

## Conformance Tests Covered

```text
- verified retrieval は checksum が一致すると bytes を返す
- verified retrieval は tampered range を specific error で拒否する
- doc_id resolution は Document Index と in-memory reader に一致する
- evidence_ref end-to-end workflow は expected bytes を restore/verify する
- concurrent verified reads は serial reads と同じ結果を返す
```

## TDD Plan

失敗する test を先に書きます。

```text
- read_range_verified は expected checksum が一致すると bytes を返し、一致しないと specific error を返す
- read_document_verified は Document Index 経由で doc_id を resolve し、per-document checksum を verify する
- file-backed doc_id verified read は in-memory reader と一致する
- evidence_ref E2E test が通る: pack, pointer build, path + container_id で reopen, range/document の verified restore
- 1 reader からの N parallel verified range reads は N serial reads と一致する
- corrupted container byte は matching verified read を fail させ、wrong bytes を silent に返さない
```

## Implementation Tasks

```text
1. reader に base read_document(doc_id) を追加する（Document Index で doc_id を byte range に resolve し read_range する）。現状は private verify helpers だけで public API はない
2. shared decode/verify core の上に read_range_verified(range, expected_checksum) を追加する
3. read_document の上に read_document_verified(doc_id, expected_checksum) を追加する
4. file-backed doc_id resolution と in-memory path が同じ Document Index lookup を共有するようにする
5. Section 3 evidence_ref JSON workflow を mirror する examples/ program を書く
6. path + container_id で reopen する end-to-end integration test を追加する
7. 1 reader から parallel verified reads を発行する concurrency test を追加する
8. evidence-retrieval API と Memory Pager integration pattern を文書化する
```

## Rust Notes

Verified reads は restored bytes に対して BLAKE3 を計算し expected value と比較します。checksum logic が
duplicated されないよう、shared decode/verify core を再利用します。Document Index は original bytes 上の
navigation structure であり、source of truth ではありません。deep verify は引き続きそれを再計算しなければ
なりません。Concurrency は Phase15 の positioned-read design を土台にし、shared mutable seek state なしで
`&self` reads が sound になるようにします。example は小さく、`cargo run --example` で runnable にし、
Phase20 public API だけを consume します。

## Review Gates

この Phase を done にする前に code review を完了しなければなりません。

この Phase を done にする前に architecture review を完了しなければなりません。

どちらかの review で spec ambiguity や library constraint が見つかった場合は、続行前に
spec とこの phase plan を更新します。

## Self-Review Checklist

```text
- verified retrieval は checksum mismatch で fail closed（specific error）するか
- doc_id resolution は Document Index を authority ではなく navigation として使っているか
- example は実際の Section 3 evidence_ref workflow を E2E で証明しているか
- concurrent verified reads は sound で serial reads と一致するか
- single tampered byte は確実に verified-read error として surface されるか
- container format byte change を避けたか
```

## Done Criteria

```text
- read_range_verified と read_document_verified が実装・test 済み
- file-backed doc_id verified read が in-memory reader と一致する
- examples/ evidence_ref workflow program が存在し実行できる
- end-to-end integration test が通る
- concurrency test が通る
- evidence-retrieval API と Memory Pager integration pattern が文書化済み
- code review findings が修正済み
- architecture review findings が修正済み
- status.md が更新済み
```

## 状態

Complete。

完了日: 2026-06-08

実装範囲:

```text
- QztReader / QztFileReader に read_range_verified、read_document、read_document_verified を追加。
- evidence_ref example と concurrent file-backed verified-read coverage を追加。
```

検証:

```text
- cargo test --test phase21_evidence_retrieval
- cargo run --example evidence_ref
- make check
```

Review notes:

```text
- Self-review pass 1 completed: verified reads は caller-provided BLAKE3 checksum が一致する場合だけ bytes を返す。
- Self-review pass 2 completed: document lookup は Document Index 経由で doc_id を resolve し、concurrent ReadAt reads は shared seek state を使わない。
- Code review completed: tampered expected checksums は VerifiedChecksumMismatch で fail closed。
- Architecture review completed: Memory Pager integration proof は Phase20 public API を consume し、original-byte evidence semantics を維持。
```

依存: Phase15（file-backed reader）、Phase20（example が使う stable public surface）、
Phase10 Document Index（完了）。この Phase は製品を定義する operation を届け、headline Memory Pager
use case を証明します。
