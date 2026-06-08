# Phase19: Resource Governance and Large-Input Hardening

[English](Phase19.md)

## 目的

adversarial input と very large input に対する残りの trust-boundary gaps を閉じます。現在の CBOR
decoder は hardcoded allocation limits（`MAX_PHASE1_ALLOCATION`, `MAX_PHASE1_ITEMS`）を使っており、
`ResourceLimits` から切断されています。そのため caller の custom limits は CBOR decoding に届きません。
search planner は `max_search_results` を強制していません。fuzz coverage は deterministic smoke harness
であり、長時間 campaign ではありません。

この Phase は resource limits をすべての decode path に通し、search result caps を強制し、
per-operation peak-memory guarantees を文書化し、fuzzing を large / streaming input へ拡張します。

この Phase は format bytes を変更してはいけません。既存構造に対する enforcement と documentation を
強化するだけです。

## Minimum MVP

```text
- CBOR decode は ResourceLimits 由来の allocation/items budget を受け取り、hardcoded constants を source of truth にしない
- open_with_limits はその budget を CBOR validation に伝播する
- search planner は max_search_results（Section 33）を強制する
```

## Goal MVP

```text
- Phase9 deterministic smoke を超える open + verify の cargo-fuzz target
- large-input property tests: in-memory budget を超える input に対する streaming writer/reader round-trip
- open, range, line, verify（quick/normal/deep）, search の per-operation peak-memory guarantees を文書化する
- adversarial fixtures: oversized declared sizes, deep CBOR nesting, very many chunks, very many dictionaries
```

## Spec refs

```text
- Section 23 error taxonomy
- Section 33 search limits, including max_search_results
- resource limits and bounded decompression の section
- Section 9.1 reader open trust boundary
```

## Conformance Tests Covered

```text
- custom ResourceLimits allocation budget は allocation 前に oversized CBOR block を拒否する
- max_search_results は returned hits を cap し、report を capped として mark する
- oversized declared sizes は allocation 前に specific errors で拒否される
- deeply nested / oversized CBOR は stack/heap blowup なしで拒否される
- streaming round-trip は in-memory budget より大きい input で成立する
```

## TDD Plan

失敗する test を先に書きます。

```text
- small allocation budget の open_with_limits は otherwise-valid large CBOR block を拒否する
- hardcoded CBOR constants ではなく ResourceLimits が behavior を gate する
- max_search_results を超える search は capped され flag が立つ
- huge uncompressed size を宣言する adversarial container は allocation 前に拒否される
- deeply nested CBOR input は specific error で拒否され、panic や unbounded recursion にならない
- configured in-memory budget を超える input で streaming write-then-read round-trip が成功する
```

## Implementation Tasks

```text
1. ResourceLimits から CBOR decoder へ budget を通し、MAX_PHASE1_ALLOCATION / MAX_PHASE1_ITEMS を source of truth から外す
2. open_with_limits からすべての CBOR validation call へ limits を伝播する
3. planner で max_search_results を強制し、SearchReport に capping を反映する
4. oversized sizes, deep nesting, many chunks, many dictionaries の adversarial fixtures を追加する
5. open + verify の cargo-fuzz target を追加する
6. streaming writer/reader を使った large-input property tests を追加する
7. per-operation peak-memory guarantees を文書化する
```

## Rust Notes

CBOR allocation と recursion depth は `ResourceLimits` から bound し、caller が recompile なしに
limits を tight/loose にできるようにします。file から読んだ size/count は allocation を駆動する前に
budget と照合しなければなりません。fuzz target は CI reproduction のため deterministic-seedable にしつつ、
local では長時間実行できるようにします。large-input tests が input 全体を memory に持たないように、
Phase15/Phase17 streaming paths を再利用します。

## Review Gates

この Phase を done にする前に code review を完了しなければなりません。

この Phase を done にする前に architecture review を完了しなければなりません。

どちらかの review で spec ambiguity や library constraint が見つかった場合は、続行前に
spec とこの phase plan を更新します。

## Self-Review Checklist

```text
- caller-supplied ResourceLimits は実際に CBOR allocation へ届くか
- file-derived size はすべて allocation 前に budget と照合されているか
- max_search_results は強制され capped として surface されるか
- adversarial input は specific errors で fail し panic しないか
- per-operation peak-memory guarantees は文書化され test されているか
- format byte change を避けたか
```

## Done Criteria

```text
- ResourceLimits-driven CBOR allocation budget が実装・test 済み
- max_search_results enforcement が実装・test 済み
- adversarial fixtures が通る（specific errors で拒否、panic なし）
- cargo-fuzz open+verify target が存在する
- large-input streaming round-trip property tests が通る
- per-operation peak-memory guarantees が文書化済み
- code review findings が修正済み
- architecture review findings が修正済み
- status.md が更新済み
```

## 状態

Pending。

依存: Phase15 と Phase17（streaming paths により full buffering なしの large-input tests が可能になる）。
M-1 CBOR-limits-wiring follow-up を解決し、search result cap と adversarial hardening を追加します。
