# QZT 実装タスク

[English](README.md)

このディレクトリは QZT 参照実装の実行計画です。

参照実装は Rust で書きます。QZT は binary layout、checked arithmetic、明示的 error、bounded decompression、testable invariant が重要なため、Rust の型と所有権モデルが適しています。

## 運用ルール

すべての Phase は TDD で進めます。

```text
1. failing test を書く、または更新する
2. 通すための最小実装を入れる
3. targeted test を実行する
4. 触った範囲の broader verification を実行する
5. diff を self-review する
6. code review を行う
7. architecture review を行う
8. review finding を修正する
9. tasks/status.md を更新する
```

tests、self-review、code review、architecture review、review fix、status update が完了するまで Phase を Complete にしてはいけません。

## 実装フロー

```text
implement -> self-review -> code review -> architecture review -> fix -> verify -> update status
```

Self-review では以下を確認します。

```text
- spec invariant を直接実装しているか
- file data を信頼する前に overflow / resource limit を検査しているか
- error が conformance test で検証できる粒度か
- success case と corruption case の両方を test しているか
- exact export semantics を壊していないか
- Core に extension behavior を混ぜていないか
```

## Review gates

すべての Phase は完了前に code review と architecture review を含めます。

Code review:

```text
- untrusted input path に hidden panic がない
- error が specific で testable
- success / corruption / boundary case が test されている
- public API が小さく一貫している
- Rust の ownership / type safety idiom に沿っている
```

Architecture review:

```text
- module boundary が spec section と対応している
- Core behavior が optional extension と結合していない
- exact export と source-of-truth semantics が保たれている
- trust boundary で resource limit と checked arithmetic が効いている
- 後続 Phase が既存実装を書き直さずに積める
```

仕様の曖昧さや library constraint が出た場合は、`docs/QZT_v0.1_Core_Spec.md` と該当 `tasks/PhaseN.md` の両方を更新します。

## Rust style expectations

```text
- offset / size / chunk ID / line ID / granule ID は必要に応じて newtype 化
- fallible operation は Result<T, QztError>
- offset / size arithmetic は checked_add / checked_mul
- fixed binary structure は TryFrom または明示 decode
- ReadAt / WriteAt trait は testability に効く範囲で使う
- fixed layout parsing は安全な borrowed slice を優先
- decompression / CBOR decode 前に Vec allocation limit を確認
- round-trip / checked arithmetic は property test
- conformance file は golden fixture
```

Parser、verifier、reader では hidden panic を避けます。壊れた file は異常系ではなく通常の入力です。

## Phase file contract

各 `PhaseN.md` は以下を持ちます。

```text
- Purpose
- Minimum MVP
- Goal MVP
- TDD plan
- Implementation tasks
- Self-review checklist
- Done criteria
- Status
```

Minimum MVP は最初に land すべき最小の有用 increment です。Goal MVP は次 Phase に進む前の intended stopping point です。

## Status tracking

進捗の single summary は [status.md](status.md) / [status.ja.md](status.ja.md) です。

## Phase order

```text
Phase0  Project foundation and quality gates
Phase1  Deterministic CBOR, primitives, and errors
Phase2  Header, footer trailer, and physical ranges
Phase3  Metadata, footer payload, index root, and chunk table skeleton
Phase4  UTF-8 chunker and sparse Chunk Table writer
Phase5  No-dictionary zstd writer and finish
Phase6  Reader open/info/export and verification levels
Phase7  Sparse line index, range reads, and CLI access
Phase8  Dictionaries, resource limits, and Reader Core completion
Phase9  Core conformance hardening and release readiness
Phase10 Dense Line Index, Document Index, memory profile, and maintenance command scoping
Phase11 Search granules and raw token index MVP
Phase12 N-gram index, planner, and benchmark reporting
Phase13 Search sidecar and high-performance search goal MVP
```

Core conformance が安定するまで Search Extension 実装は開始しません。ただし design-only work は例外です。
