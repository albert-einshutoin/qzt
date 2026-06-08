# Phase16: Streaming Verification and Export

[English](Phase16.md)

## 目的

deep verification と export から O(file size) のメモリコストを取り除きます。現状の
`verify_deep` は全 chunk を decode し、container checksum と line information を再計算する前に
original byte stream 全体を `Vec<u8>` に蓄積します。大きな file では peak memory が倍化します。

この Phase では deep verify と export を streaming pass にします。BLAKE3 hasher と line analyzer
を incremental に更新し、cross-chunk continuation state（newline / UTF-8 boundary check に必要な
previous chunk tail）だけを小さく保持します。

この Phase は verification guarantee を弱めてはいけません。以前 check していたすべての byte は
引き続き check されなければなりません。変わるのは memory profile だけです。

## Minimum MVP

```text
- verify_deep は stream する: chunk decode -> container BLAKE3 hasher update -> line/newline analysis update -> decoded buffer drop
- cross-chunk line continuation は retained previous-chunk tail だけで検証する
- verify_deep は full original bytes を Vec に蓄積しない
```

## Goal MVP

```text
- export_to は chunk-by-chunk に writer へ stream し bounded buffer を使う
- file-backed reader 上の verify_deep は bounded memory を使う
- Document Index deep verification は全 document materialize ではなく range-scoped（または separate deep-document mode）
- deep verify peak memory が file size ではなく max chunk size に bound される test がある
```

## Spec refs

```text
- verification levels（quick / normal / deep）の section
- Section 13 line semantics and continuation flags
- Section 28 Document Index verification
```

## Conformance Tests Covered

```text
- deep verify は compressed-chunk checksum mismatch を引き続き検出する
- deep verify は uncompressed-chunk checksum mismatch を引き続き検出する
- deep verify は container checksum mismatch を引き続き検出する
- deep verify は stale Dense Line Index を引き続き検出する
- deep verify は stale Document Index range を引き続き検出する
- streaming deep verify result は全 fixture で旧 full-buffer result と一致する
```

## TDD Plan

失敗する test を先に書きます。

```text
- streaming verify_deep は全 fixture で旧実装と同じ Ok/Err result を返す
- verify_deep peak allocation は allocation probe で max chunk size + index に bound される
- continuation state を buffer 全体ではなく carried state で扱っても corrupted final chunk が検出される
- export_to は full original size を allocate せず stream する
- Document Index deep verify は全 document materialize なしで stale range を拒否する
```

## Implementation Tasks

```text
1. verify_deep の full-output Vec を incremental BLAKE3 hasher に置き換える
2. full-buffer line analysis を previous-chunk tail だけを持つ incremental line/newline analyzer に置き換える
3. STARTS_WITH_LINE_CONTINUATION flag を whole buffer ではなく retained tail で検証する
4. export_to を bounded reusable buffer で chunk-by-chunk streaming にする
5. Document Index deep verify を per-document ranges に scope する
6. deep verify peak memory を bound する allocation-probe test を追加する
7. 全 fixture で旧挙動との equivalence を確認する
```

## Rust Notes

final buffer を hash するのではなく、`blake3::Hasher::update` を incremental に使います。
per-chunk reallocation を避けるため、maximum uncompressed chunk size に合わせた reusable decode
buffer を 1 つ持ちます。continuation analyzer が必要とするのは previous chunk の last byte
（UTF-8 / CRLF safety のための最小 trailing bytes）だけです。それ以上保持しません。
Streaming 化しても corruption cases の error variants を変えてはいけません。

## Review Gates

この Phase を done にする前に code review を完了しなければなりません。

この Phase を done にする前に architecture review を完了しなければなりません。

どちらかの review で spec ambiguity や library constraint が見つかった場合は、続行前に
spec とこの phase plan を更新します。

## Self-Review Checklist

```text
- streaming deep verify は buffered version と同じ byte を check しているか
- すべての corruption cases が同一 error variants で検出されるか
- retained cross-chunk state は最小（tail only）か
- peak memory は file size ではなく chunk size に bound されているか
- export は full-original allocation なしで stream しているか
- quick / normal verify behavior を変えていないか
```

## Done Criteria

```text
- streaming verify_deep equivalence tests が全 fixture で通る
- deep verify peak-memory bound test が通る
- export streaming test が通る
- Document Index range-scoped deep verify test が通る
- code review findings が修正済み
- architecture review findings が修正済み
- status.md が更新済み
```

## 状態

Pending。

依存: Phase15。file-backed reader が ReadAt path を提供することで、大きい file に対する
bounded-memory deep verify が意味を持ちます。hasher / analyzer refactor は in-memory reader で
開始し、後で両 reader に適用できます。
