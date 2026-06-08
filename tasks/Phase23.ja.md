# Phase23: Acceptance Threshold Harness

[English](Phase23.md)

## 目的

`docs/QZT_v0.1_Validation_Corpus.md` を executable gate にします。C1-C6 validation corpora を
deterministically に生成し、non-evidence HARD invariants を tests として assert し、SOFT targets を
provisional expectation bands として記録して regressions を可視化します。この harness は Phase18
（competitive timing）と Phase22（golden vectors）が再利用する corpus generators を所有します。
Evidence retrieval は、Phase21 が verified evidence API を届けた後に Phase23b extension として追加します。

この Phase は measurement と acceptance criteria を追加します。threshold を通すために container format
bytes や reader/writer behavior を変更してはいけません。

## Minimum MVP

```text
- C1-C6 corpora の deterministic, seeded generators（docs/QZT_v0.1_Validation_Corpus.md 参照）
- Phase23a HARD invariants を corpus ごとに assert: lossless round-trip, range-restore byte bound, deep-verify corruption detection
- provisional SOFT targets（compression ratio, search decode ratio, peak memory）を corpus ごとに記録する report
```

## Goal MVP

```text
- SOFT targets は documented expectation bands と比較される。out-of-band results は明確な message で flag され、silent pass も hard-fail もしない
- Phase23b evidence-retrieval HARD invariants（clean verified 100%, tampered failure 100%）は Phase21 後に C1 で assert される
- small corpus sizes の HARD invariants は default quality gate に入り、large sizes は opt-in flag の背後に置く
- report format は run 間比較ができる程度に stable
```

## Spec refs

```text
- docs/QZT_v0.1_Validation_Corpus.md corpus taxonomy and acceptance thresholds
- Section 35 test suite
- verification levels の section
```

## Conformance Tests Covered

```text
- lossless round-trip は C1-C6 全 corpora で成立する
- range restore は各 corpus で documented byte bound 内で decode する
- corruption sweep は single-byte corruptions を 100% correct error で検出する
- Phase23b: evidence retrieval は Phase21 後に clean reads を verify し tampered reads で fail closed する
- SOFT metrics は各 corpus で記録され band-checked される
```

## TDD Plan

失敗する test を先に書きます。

```text
- 各 corpus generator は deterministic: same seed は byte-identical corpus を生成する
- export(pack(corpus)) == corpus が C1-C6 全 corpora で成立する
- 各 corpus の range restore は requested_size + 2 * chunk_size を超えて decode しない
- chunk, metadata, index bytes の corruption sweep は documented error で 100% 検出される
- Phase23b: Phase21 後に clean evidence read は verify し tampered evidence read は fail closed する
- expectation band 外の SOFT metric は明確な message で flag される（hard-fail ではない）
```

## Implementation Tasks

```text
1. C1-C6 の deterministic seeded generators を作る
2. Phase23a HARD invariants（round-trip, range bound, corruption detection）を corpus ごとに assert する
3. SOFT targets（compression ratio, search decode ratio, peak memory）を corpus ごとに測定する
4. SOFT targets を documented provisional bands と比較し、out-of-band results を flag する
5. stable で比較可能な report を emit する
6. small-size HARD invariants を make check に接続し、large-size runs は opt-in flag の背後に置く
7. Phase18 と Phase22 が再利用する shared module として generators を expose する
8. Phase21 後に Phase23b C1 evidence clean/tampered invariants を同じ harness に追加する
```

## Rust Notes

pack/export を再実装せず writer と reader を再利用します。corpora が byte-identically regenerate されるよう、
seed は明示します。small-corpus HARD invariants は default gate に入れます。large-corpus と timing-sensitive
runs は Phase18 と同じ opt-in mechanism の背後に置き、default `make check` を速く保ちます。SOFT targets は
recorded evidence です。out-of-band は flag と investigation prompt であり、自動 build failure ではありません。

## Review Gates

この Phase を done にする前に code review を完了しなければなりません。

この Phase を done にする前に architecture review を完了しなければなりません。

どちらかの review で spec ambiguity や library constraint が見つかった場合は、続行前に
spec とこの phase plan を更新します。

## Self-Review Checklist

```text
- 6 corpora はすべて deterministic で byte-identically regenerate できるか
- HARD invariants は全 corpora で成立するか
- SOFT targets は provisional として band-checked / flagged され、silent pass していないか
- small-corpus HARD invariants は default gate で大きく遅くならずに動くか
- generators は Phase18 / Phase22 と shared され、duplicated されていないか
- threshold を通すための format change や behavior change を避けたか
```

## Done Criteria

```text
- deterministic C1-C6 generators が存在し shared されている
- HARD invariant tests が全 corpus で通る
- corruption sweep detection が 100%
- Phase23b evidence clean/tampered invariants は Phase21 後に通る
- SOFT targets は docs/QZT_v0.1_Validation_Corpus.md の provisional bands に対して記録・band-check される
- small-size HARD invariants は make check に入り、large-size runs は opt-in
- code review findings が修正済み
- architecture review findings が修正済み
- status.md が更新済み
```

## 状態

Pending。

依存: Phase23a は Phase15 に依存します（peak-memory / seek bounds の file-backed reader のため）。
Phase23b は C1 evidence invariants のために Phase21 の evidence-retrieval API に依存します。Phase18 と
Phase22 に corpora を提供します。Phase15 の直後に Phase23a を実行し、competitive / vector work が
同じ corpora を使う前に acceptance thresholds を整えます。
