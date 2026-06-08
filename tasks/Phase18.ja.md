# Phase18: Competitive Benchmark Harness

[English](Phase18.md)

## 目的

未検証の product question「なぜ既存ツールではなく QZT なのか」に答えます。Release hardening note は、
SQLite FTS、Tantivy、Lucene、seekable zstd、split-frame object storage との比較が存在しないことを
明記しています。その evidence がなければ、製品の採用理由は証明されません。

この Phase では common corpus 上の再現可能な comparative benchmark を構築します。測定するのは、
QZT が得意だと主張する operation です: random range restore、single-line restore、
original-byte verification 付き evidence-position search、search sidecar を含む on-disk size。

この Phase は測定だけを行います。数値を良く見せるために format や reader/writer behavior を
変更してはいけません。

## Minimum MVP

```text
- deterministic corpus generator に large option（>= 100 MB）を持たせ、in-memory baselines に実際の stress をかける
- QZT vs raw-zstd-whole-file: random range restore latency と bytes decompressed
- methodology を文書化する: corpus, environment capture, exact reproduction command
- 結果を docs/ に記録し、SLA ではなく local evidence と明示する
```

## Goal MVP

```text
- 同じ corpus で QZT sidecar search vs SQLite FTS5 / ripgrep: query latency, index build time, index size ratio, identical hit-set correctness
- QZT vs seekable zstd の random access latency
- 数値から導いた honest "when NOT to use QZT" section（observed sidecar-larger-than-source result を含む）
- benchmark は opt-in（feature flag または ignored test）で、default quality gate を速く保つ
```

## Spec refs

```text
- format spec section はない。docs/QZT_v0.1_Release_Hardening.md の "Remaining Product Evidence Gap" を参照する。
```

## Conformance Tests Covered

```text
- 直接の conformance test はない。この Phase は format conformance ではなく external product evidence を作る
- correctness cross-check: QZT search と reference tool は shared corpus 上で同じ hit set を返す
```

## TDD Plan

失敗する tests/checks を先に書きます。

```text
- corpus generator は deterministic: same seed は identical bytes を生成する
- QZT range restore は ground-truth slice と一致する exact requested bytes を返す
- QZT search hit set は fixed query set で reference-tool hit set と一致する（timing 前の correctness gate）
- benchmark runner は optional external tool がなくても panic せず required metrics を記録する
- size-ratio reporting は container size と sidecar size を分けて記録する
```

## Implementation Tasks

```text
1. size parameter を持つ deterministic large-corpus generator を追加する
2. bytes-decompressed accounting 付き QZT-vs-raw-zstd random range restore benchmark を実装する
3. external-tool comparisons を gate する opt-in feature flag（例: "bench-compete"）を追加する
4. SQLite FTS5 と ripgrep 比較をその flag の背後に統合し、tool missing 時は graceful skip する
5. timing を信頼する前に hit-set correctness cross-check を追加する
6. environment metadata（CPU, OS, toolchain, tool versions）を report に捕捉する
7. 結果と "when NOT to use QZT" section を docs/ に記録する
8. default make check path から comparison を外す
```

## Rust Notes

external tools（sqlite3, ripgrep）は documented optional system dependencies として Cargo feature の
背後で呼び出し、それらがない CI でも通るようにします。公平な large-file QZT numbers には Phase15 が
必要です。in-memory reader で comparison を実行すると、測る対象が間違います。timing は environment
capture 付き evidence として報告し、guaranteed threshold として扱いません。faster-but-wrong result が
勝ちに見えないように、correctness gate を timing より先に置きます。

## Review Gates

この Phase を done にする前に code review を完了しなければなりません。

この Phase を done にする前に architecture review を完了しなければなりません。

どちらかの review で spec ambiguity や library constraint が見つかった場合は、続行前に
spec とこの phase plan を更新します。

## Self-Review Checklist

```text
- corpus は deterministic で、memory/I/O に stress をかける十分な大きさか
- comparison は timing 前に correctness を gate しているか
- external-tool comparisons は opt-in / skippable で default gate を速く保つか
- environment が捕捉され reproducible か
- report は sidecar size を含め、QZT が負ける場所を正直に書いているか
- QZT numbers は in-memory reader ではなく file-backed reader から取っているか
```

## Done Criteria

```text
- deterministic large-corpus generator が存在する
- QZT vs raw-zstd range-restore benchmark が実行され metrics を記録する
- QZT vs SQLite FTS5 / ripgrep search comparison が feature flag の背後で実行される
- search hit-set correctness cross-check が通る
- 結果と "when NOT to use QZT" section が docs/ に記録される
- comparison は default quality gate から除外されている
- code review findings が修正済み
- architecture review findings が修正済み
- status.md が更新済み
```

## 状態

Pending。

依存: Phase15（公平な large-file QZT numbers には file-backed reader が必要）と Phase23a
（shared C1-C6 corpus generators と acceptance thresholds）。product assessment と Release
Hardening note で特定された competitive-validation gap を閉じます。
