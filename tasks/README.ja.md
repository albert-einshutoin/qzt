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

Optional indexes と extension profiles は、Phase が release target に含むと明示しない限り、Core release readiness を block してはいけません。

## Product Completeness Track (post-v0.1)

Phase0-Phase13 は format-complete な v0.1 reference implementation を届けます。Product Completeness
Track は maturity を "reference implementation / technical preview" から spec の product goal、
つまり Memory Pager や AI memory systems に embed される Cold Evidence Container へ近づけます。
これらの Phase は process、scalability、integration work であり、container format bytes や
`export(pack(input)) == input` invariant は変更しません。

この track には 2 つの sub-track があります。engine sub-track は I/O model と hardening を
production-credible にします。consumer sub-track は、QZT を外部 system が embed できる stable で
verifiable な dependency にします。

Engine sub-track:

```text
Phase14 Open-source release hygiene: LICENSE, CI, package metadata, contributor docs
Phase15 File-backed seeking reader (QztFileReader): bounded-memory open/range/line/export over file paths
Phase16 Streaming verification and export: verify_deep と export から O(file size) memory を除去
Phase17 Streaming writer (QztFileWriter): RAM より大きい container を build、pack_bytes と byte-identical
Phase18 Competitive benchmark harness: QZT vs raw zstd, seekable zstd, SQLite FTS5, ripgrep
Phase19 Resource governance and large-input hardening: ResourceLimits を CBOR に配線、search result cap、fuzz 拡張
```

Consumer sub-track:

```text
Phase20 Public API stabilization: lib.rs surface curate, writer builder, missing_docs, semver/stability policy, surface snapshot test
Phase21 Verified evidence retrieval and Memory Pager integration: read_document / read_range_verified / read_document_verified, evidence_ref example, concurrent verified reads
Phase22 Portable conformance vectors and format stability: golden .qzt vectors, vector runner, third-party verification, frozen v0.1 format-stability statement
```

Validation (cross-cutting):

```text
Phase23 Acceptance threshold harness: deterministic C1-C6 corpora, HARD invariants asserted, SOFT targets band-checked (see docs/QZT_v0.1_Validation_Corpus.md)
```

`docs/QZT_v0.1_Validation_Corpus.md` は、QZT を何の text に対して validate し、どの result を
expectations を満たすものと判定するかを定義します（HARD invariants と SOFT target bands）。
Phase23a はその doc を executable にし、Phase18 と Phase22 が再利用する corpus generators を所有します。
Phase23b は Phase21 の verified evidence API が land した後に evidence invariants を追加します。

Dependency order:

```text
Phase14 -> independent, land first
Phase15 -> foundation for Phase16, Phase17, Phase18, Phase21, Phase23a
Phase16 -> depends on Phase15
Phase17 -> depends on Phase15
Phase18 -> depends on Phase15 and Phase23a (shared corpora)
Phase19 -> depends on Phase15 and Phase17
Phase20 -> depends on Phase14; prerequisite for Phase21 and Phase22
Phase21 -> depends on Phase15 and Phase20; adds the base read_document API and concurrent verified reads
Phase22 -> depends on Phase20, Phase23a, and the Phase9 conformance map
Phase23a -> depends on Phase15; builds C1-C6 generators and HARD invariants that do not require evidence APIs
Phase23b -> depends on Phase21; adds C1 evidence-retrieval invariants to the same harness
```

Recommended sequence: Phase14、次に Phase15。その後は sub-tracks を parallel に進めます。
Engine は Phase16 と Phase17 をどちらの順でも進め、Phase23a corpora が存在した後に Phase18、
その後 Phase19。Consumer は Phase20、Phase21、Phase22 の順。Validation は Phase15 の直後に
Phase23a を実行し、Phase21 が land したら Phase23b で拡張します。

これらの Phase は container format bytes を変更してはいけません。byte layout を変える必要がある変更は、
この track ではなく新しい format version に属します。

## Post-Phase23 実行 (post-v0.1 ロードマップ)

Phase0-Phase23 は完了しています。実行は 2 本の GitHub issue ロードマップで継続します。
リファクタリング・ロードマップ（issue #31、issue #2-#30）とプロダクト価値ロードマップ
（issue #47、issue #33-#46）です。トラック横断の順序、wave 計画、並走制約、
マイルストーン、リリースゲートは [PostPhase23.ja.md](PostPhase23.ja.md) で固定します。
詳細な手順と受け入れ基準は issue 側にあります。この README のルール（TDD loop、
review gates、format bytes 変更禁止）はすべての issue PR に引き続き適用されます。
