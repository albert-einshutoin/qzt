# QZT タスク進捗

[English](status.md)

確認日: 2026-09-26。基準main: `4d9ceed420918faed3871fa47b86511dee9afff5`。

## 現在の判断先

現在の優先度、Issue間の担当境界、保留判断は[ロードマップ #31](https://github.com/albert-einshutoin/qzt/issues/31)が正本です。各子IssueとPRに受入条件と完了証拠を置き、このページは進捗の短い入口とします。開発コマンドは[AGENTS.md](../AGENTS.md)と[CONTRIBUTING.ja.md](../CONTRIBUTING.ja.md)、公開準備は[release checklist](../docs/RELEASE.ja.md)を参照してください。

## Phase summary（当時のスコープで完了）

| Phase | 名前 | 状態 | Minimum MVP | Goal MVP |
|---:|---|---|---|---|
| 0 | Project foundation and quality gates | Complete | Rust crate、local quality command、test harness | 再現可能な quality gate と fixture layout |
| 1 | Deterministic CBOR, primitives, and errors | Complete | canonical CBOR rejection と fixed primitive | typed errors、checked arithmetic、property tests |
| 2 | Header, footer trailer, and physical ranges | Complete | fixed structure encode/decode | range validator と corruption tests |
| 3 | Metadata, footer payload, index root, and chunk table skeleton | Complete | deterministic CBOR schema と empty skeleton | zstd chunk なしの structural verifier |
| 4 | UTF-8 chunker and sparse Chunk Table writer | Complete | deterministic Chunk Plan | CRLF-safe、continuation-aware line metadata |
| 5 | No-dictionary zstd writer and finish | Complete | simple UTF-8 の pack/export equality | zstd frames、BLAKE3、Footer finish、pack metric |
| 6 | Reader open/info/export and verification levels | Complete | valid file の open/info/export | quick/normal/deep verify corruption coverage |
| 7 | Sparse line index, range reads, and CLI access | Complete | read_range と read_line_raw | CLI range/line、spanning-line、benchmark |
| 8 | Dictionaries, resource limits, and Reader Core completion | Complete | embedded dictionary fixture を読む | Reader Core と resource hardening |
| 9 | Core conformance hardening and release readiness | Complete | Core test pass と fuzz smoke | v0.1 Core release candidate |
| 10 | Dense Line Index, Document Index, memory profile, maintenance scope | Complete | Dense Line Index acceleration | Document Index、memory profile、maintenance decision |
| 11 | Search granules and raw token index MVP | Complete | Raw token index | verified token search with metrics |
| 12 | N-gram index, planner, benchmark reporting | Complete | Raw n-gram candidate search | rarest-first planner と performance reports |
| 13 | Search sidecar and high-performance search goal MVP | Complete | `.qzi` sidecar validation | memory-mappable high-performance search flow |

## Product Completeness Track（当時のスコープで完了）

これらの Phase は、Memory Pager や AI memory systems に embed される Cold Evidence Container という
spec の product goal に向けて maturity を上げます。container format bytes は変更しません。この track は
engine sub-track (14-19) と consumer sub-track (20-22) を持ちます。

Engine sub-track:

| Phase | 名前 | 状態 | Minimum MVP | Goal MVP |
|---:|---|---|---|---|
| 14 | Open-source release hygiene | Complete | LICENSE、CI running make check、package metadata | Contributor docs、MSRV matrix、doc build、packageability check。crates.io publish dry-run は Phase20 後まで defer |
| 15 | File-backed seeking reader | Complete | ReadAt trait と、index region だけ読む QztFileReader open | Bounded-memory range/line/export、CLI file reader 接続 |
| 16 | Streaming verification and export | Complete | full-original Vec なしの streaming verify_deep | Bounded-memory export と file-backed deep verify |
| 17 | Streaming writer | Complete | QztFileWriter push/finish | pack_bytes と byte-identical、bounded memory、streaming pack CLI |
| 18 | Competitive benchmark harness | Complete | large corpus で QZT vs raw-zstd range restore | `bench-compete` 配下の QZT vs SQLite FTS5 / ripgrep correctness gate |
| 19 | Resource governance and large-input hardening | Complete | ResourceLimits-driven CBOR budget、max_search_results cap | cargo-fuzz open+verify target、large-input acceptance coverage、documented memory bounds |

Consumer sub-track:

| Phase | 名前 | 状態 | Minimum MVP | Goal MVP |
|---:|---|---|---|---|
| 20 | Public API stabilization | Complete | default では hidden internal modules、curated crate-root surface、writer builder | missing_docs lint、semver/stability policy、root API smoke test、docs.rs |
| 21 | Verified evidence retrieval and Memory Pager integration | Complete | read_range_verified / read_document_verified、evidence_ref example | E2E integration test、doc_id resolution、concurrent verified reads、documented integration pattern |
| 22 | Portable conformance vectors and format stability | Complete | Golden .qzt vectors、manifest、vector runner | Core map + corruption coverage、third-party procedure、frozen v0.1 format-stability statement |

Validation (cross-cutting):

| Phase | 名前 | 状態 | Minimum MVP | Goal MVP |
|---:|---|---|---|---|
| 23 | Acceptance threshold harness | Complete | Phase23a deterministic C1-C6 corpora、HARD invariants asserted、SOFT targets recorded | Phase23b evidence invariants on C1 after Phase21、shared generators for Phase18/22 |

当時の依存順は、完了済みPhaseの計画履歴であり、現在の作業キューではありません。Engine: 14 -> 15 -> 16/17 -> 18 -> 19。Consumer: 14 -> 20 -> 21 -> 22。Validation: 15 -> 23a、21 -> 23b。詳細は[Phase資料](README.ja.md)、受入閾値は[validation corpus](../docs/QZT_v0.1_Validation_Corpus.ja.md)を参照してください。

## 現在地

Phase 0–23は当時の実装スコープに対して完了しています。[価値ロードマップ #47](https://github.com/albert-einshutoin/qzt/issues/47)と子Issue #33–#46もclose済みです。[#31](https://github.com/albert-einshutoin/qzt/issues/31)のpreview hardening対象10件は、それぞれのIssue・PRに実装と検証の証拠があります。Phase完了だけでproduction-readyや新しい公開版とは判断しません。

[FFI監査 #307](https://github.com/albert-einshutoin/qzt/issues/307)は完了しました。製品コード修正3件、監査済みの局所Semgrep例外10件、既知の未解決検出0件です。main `4d9ceed420918faed3871fa47b86511dee9afff5`の[security CI](https://github.com/albert-einshutoin/qzt/actions/runs/36177692759)と[通常CI](https://github.com/albert-einshutoin/qzt/actions/runs/36177692136)は成功しました。これは対象SHAのscan・監査の証拠であり、`unsafe`全廃や脆弱性ゼロを意味しません。他のfuzz、OS別テスト、CIの証拠は各Issue記録のcommit・環境に紐付き、今回の基準mainで全項目を再実行したとは扱いません。

公開済みGitHub Releaseは[v0.1.0-pre.2](https://github.com/albert-einshutoin/qzt/releases/tag/v0.1.0-pre.2)です。その後のmain修正は配布済みbinaryには含まれません。Coreは**release candidate**、QZI検索と製品全体は**technical preview**で、production-readyではありません。[#295のCLI費用レポート](../docs/benchmarks/2026-09-cli-cost.md)はmacOS/Apple M4 1台の合成corpusに対する計測で、公開pre.2 binaryの性能や本番SLAではありません。

P2保守Issue 10件は[#31](https://github.com/albert-einshutoin/qzt/issues/31)で保留中であり、次回previewの必須ゲートには含めません。[PR #286](https://github.com/albert-einshutoin/qzt/pull/286)はopenで採否を別途判断します。[#27](https://github.com/albert-einshutoin/qzt/issues/27)は別途profileと採否判断が必要です。この文書同期は[#309](https://github.com/albert-einshutoin/qzt/issues/309)で追跡します。

## 次の判断

この同期後の作業は#31で明示的に選びます。候補はPR #286の扱い、#27のprofileと採否、将来のpreview公開準備です。自動的には着手しません。受入証拠は子Issue、検索の契約は[CLIリファレンス](../docs/CLI.ja.md)と[検索運用](../docs/guides/search-operations.ja.md)、Writer契約は[#293](https://github.com/albert-einshutoin/qzt/issues/293)、ファイル出力は[#290](https://github.com/albert-einshutoin/qzt/issues/290)と[FFI監査](../docs/security/ffi-output-audit.md)、公開条件は[release checklist](../docs/RELEASE.ja.md)を参照してください。[Post-Phase23計画](PostPhase23.ja.md)は2026年6月の提案履歴で、現在の着手順やrelease gateではありません。

## Completion tracks

| Track | 状態 | Notes |
|---|---|---|
| Writer Core | Complete | v0.1 Writer Core は no-dictionary output。dictionary-emitting writer は Core-ready scope 外。 |
| Reader Core | Complete | embedded dictionary reading、resource limits、partial access、verify levels が完了。 |
| Optional Core-defined indexes | Complete | Dense Line Index と Document Index は optional cache として検証済み。 |
| Search Extension | Complete | token/ngram correctness path、planner metrics、QZI sidecar lookup が完了。 |
| Release Hardening | Complete | `tests/release_hardening.rs` と release hardening note が存在。 |
| Product Completeness: engine | Complete | Phase14-Phase19。open-source hygiene、file-backed seeking reader、streaming verify/export/writer、competitive benchmarks、large-input resource governance。 |
| Product Completeness: consumer | Complete | Phase20-Phase22。curated public API、Memory Pager integration proof 付き verified evidence retrieval、portable conformance vectors と frozen format-stability statement。 |
| Product Completeness: validation | Complete | Phase23。docs/QZT_v0.1_Validation_Corpus.md の C1-C6 corpora に対する acceptance threshold harness。HARD invariants と provisional SOFT target bands で「期待値を満たす」を測定可能にする。 |

## 過去の検証記録

以下の記録は当時実行した範囲の履歴です。基準mainでの再実行を示すものではありません。現在のIssue・CI証拠は上記リンクを参照してください。


すべての Phase は `make check` または targeted tests で検証されています。2026-06-08 の Phase14-Phase23 完了時点では、`make check`、`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`、`cargo package --offline --allow-dirty` が通っています。通常の `cargo package --allow-dirty` は sandbox から crates.io に到達できず失敗しました。

design review follow-ups (DR-1..DR-6) 適用後: `cargo fmt --all -- --check`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test --all-targets --all-features` が通っています。151 テスト通過（+12）。

quality review follow-ups (2026-06-10) 適用後: search の hit verification が chunk decode cache を再利用するようになり（4,124 ヒットのクエリが 16,376 ms → 49 ms、新メトリクス `physical_decoded_bytes` が chunk レベルの復号量を可視化）、n-gram 長未満・token 化不能なクエリは silent な 0 件ではなく `incomplete_reason` と CLI 警告を返し、`qzt export` は bounded memory でストリーム出力（45 MB コーパスで最大 RSS 9.6 MB）、品質ゲートに default-features の `cargo check --lib --bins` を追加、`bench-release` を修復（Phase20 の API curation 以降コンパイル不能だった）して `--release --all-features` で実行: 2.4 MB deterministic corpus で pack 137.745 MiB/s、export 473.350 MiB/s、range 532.576 MiB/s（2026-06-07 の記録は debug build の値）。155 テスト通過（+4）。

bounded-memory search wiring（DR-7、2026-06-10）適用後: `qzt search`・`qzt info`・`qzt sidecar-rebuild` が `QztFileReader` 上で動作し、新しい `QziFileSidecar` は open 時に manifest と term dictionary のみ読み込み（各セクションは bounded buffer でストリーム検証）、posting list と候補 granule レコードはクエリごとに遅延 fetch します。42 MB / 40 万行コーパスでの before/after 実測（旧バイナリは直前コミットからビルド）: rare sidecar query 518 MB → 9.8 MB max RSS・1.33 s → 0.04 s、dense 8 万ヒット query 532 MB → 36 MB・1.11 s → 0.17 s、`qzt info` 9.6 MB → 2.0 MB。index builder はチャンク単位のストリーミング + 二分探索の chunk span 算出になり（O(lines × chunks) スキャンを除去）、再構築した sidecar バイト列は旧 builder と同一。sidecar なしの transient search と sidecar-rebuild は引き続き index 構築メモリが支配的（このコーパスで約 0.6〜1.3 GB）で、sidecar サイズ/構築の follow-up として追跡します。160 テスト通過（+5）。

post-Phase23 planning（2026-06-12）: Post-Phase23 実行計画（`tasks/PostPhase23.ja.md`）を追加。GitHub ロードマップ #31（リファクタリング、issue #2-#30）と #47（プロダクト価値、issue #33-#46）に対する wave 順序・並走ルール・マイルストーン M1-M6・リリースゲートを固定。`make check` と `git diff --check` が通っています。161 テスト通過（+1）。

refactor phase 1（2026-06-13）: リファクタ Phase 1（マージ済み #2 の上に #3-#9）を完了。実バグ #8 を修正（profile 検証を `pack_bytes_internal` に移して全 pack 経路で検証、`"memory"` は全経路で DocumentIndex 必須に統一）、`QztError` の Display を人間可読化し `NotImplemented` を `Io(ErrorKind)` / `UnsupportedIndexMode` に置換（#3）、`usize_to_u64`/`u64_to_usize` ヘルパーで定型変換約 155 箇所を置換（#4）、`Checksum::from_hasher`/`from_raw_bytes` でインライン構築を一掃（#5）、lib.rs のモジュール宣言を `internal_module!` マクロに集約（#6）、デッドコード `QztWriter`/`format::VERSION` を削除（#7）、Cargo.toml に `[lints]` clippy pedantic ベースラインを確立（#9。public API 変更: `pack_bytes_with_document_index`/`pack_bytes_with_memory_profile` は `&DocumentIndex` を、`run_release_benchmark_with_corpus` は `&[u8]` を取る）。`make check` と `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` が通っています。169 テスト通過（+8）。

value phase 1（2026-06-13）: Value Phase 1（#33-#37）を完了。`qzt info` が `container_id`/`original_checksum`/`newline_mode` を表示し `--format json` を獲得（bin 専用の `cli_json` RFC 8259 エスケープヘルパーを新設、#33）。`qzt verify` は `VerifyReport`（checked chunks / decoded bytes）を表示し、`--format json`（成功 `{"ok":true,...}` / 失敗 `{"ok":false,"error":...}` を stdout に出力し exit 1）と 0/1/2 終了コード契約を整備（#34）。新コマンド `qzt docs`（タブ区切り / JSON の Document Index 一覧、first_line は 1-based 表示）と `qzt doc`（既定で BLAKE3 検証付き取り出し、`--no-verify`、`-o`）を追加（#35）。`qzt search --format json`（エスケープ済み hits/metrics、`SearchReport`/`SearchHit`/`SearchMetrics`/`PlannerDecision` を crate ルートから re-export、#36）。`qzt pack -` が bounded-memory な `QztFileWriter` 経路で stdin をストリーミング pack（core 以外 / dense 併用は exit 2 で拒否、#37）。`make check`、docs ビルド、stdin パイプ + JSON 出力の手動スモークが通っています。216 テスト通過（+47）。

Phase14-Phase23 のセルフレビューでは以下を修正済みです。

```text
- Phase17 streaming writer の checksum 計算が compressed chunks を全保持していた問題を、seek/read による prefix hash へ修正
- Phase18 bench-compete feature 配下に ripgrep / SQLite FTS5 correctness hooks を追加
- Phase19 cargo-fuzz open+verify target を追加
- Phase20 default build で internal modules を隠し、crate-root curated API を使う構成に修正
```

直近の release blocker review fix では以下を修正し、`make check` が通っています。

```text
- multi-token token-search hit reporting
- Metadata writer option serialization
- CLI profile/dense wiring
- CLI error detail preservation
- deep verify integer conversion
- O(n log n) physical range overlap validation
- info metadata reporting
- placeholder streaming writer API の doc hidden 化
```

## 過去のレビュー指摘

以下は当時の判断です。現在の保留Issueと優先度は#31を参照してください。


Fixed:

```text
- C-1 verified_spans multi-token hits
- C-2 Metadata writer options
- C-3 CLI --profile / --dense-line-index
- C-4 CLI error detail
- H-1 deep verify integer conversion
- H-2 physical range overlap complexity
- H-5 QztWriter placeholder
- H-6 qzt info hardcoded metadata
- P1 chunker target-size soft-limit
- P1 required block validation
- P2 Metadata decode indexes/integrity
- P0/P2 README limitations
```

Deferred:

```text
- varuint duplication cleanup
- TextAnalysis / LineInfo duplication cleanup
- CBOR limits wiring
- post-v0.1 cleanup: technical-preview API の missing_docs warning を全 item docs に落とし込む
```

## 過去の設計レビュー指摘 (2026-06-08)

combined design + product review から適用しました。コンテナ format bytes の変更なし。すべて変更されていない public API の背後にある read-path / verify-path / docs / test の改善です。

| Item | 状態 | Notes |
|---|---|---|
| DR-1 README phase plan 陳腐化 | Fixed | "Phase Plan" を Phase0-13 + Product Completeness 14-23 をカバーする内容に書き直し。陳腐化していた "QztFileReader planned" 制限を、実際に残っているギャップ（search がコンテナ全体を読み込む）に修正。日本語 README も同期済み。 |
| DR-2 `QztReader` read-path 重複 | Fixed | `QztReader::read_range` / `read_line_raw` を shared free functions へ委譲し、`QztFileReader` と平行する ~75 行の重複ロジックを削除。 |
| DR-3 deep-verify document 再デコード + O(documents × chunks) | Fixed | `DocumentHasher` を追加し、deep verify の chunk loop で既にデコード済みの chunk からドキュメント範囲をワンパスでハッシュ（再デコードなし）。`document_chunk_range` は二分探索 2 回に切り替え。任意の順序・重複ドキュメントに対応。空ドキュメントはデコード不要。ユニット + 統合テスト追加。 |
| DR-4 `read_document` O(documents) scan + 未使用 `doc_id_hash` | Fixed | `SkeletonDetails` が open 時に `doc_id -> index` の HashMap を一度構築。`find_document` が O(1) になり、重複 id では先着優先を維持。 |
| DR-5 `find_document` エラー混在 | Fixed | `QztError::DocumentNotFound` を追加。「document index なし」（`MissingRequiredBlock`）と「id 未登録」（`DocumentNotFound`）が区別可能に。 |
| DR-6 プロパティカバレッジ薄さ + 不使用パラメータ | Fixed | `tests/property_roundtrip.rs` を追加（`export(pack(x)) == x`、`read_range == slice`）。未使用の `StreamingTextAnalysis::new` パラメータを削除。 |
| DR-7 search でメモリ読み込みリーダー使用 (P-2) | Fixed | 2026-06-10: `search_file` / `build_from_file` / `build_search_sidecar_from_file` / `QziFileSidecar` により search と sidecar lookup を `QztFileReader` へ接続（posting/granule は遅延 fetch）。CLI の search/info/sidecar-rebuild が file-backed パスを使用。既存の `&[u8]` / `&QztReader` エントリポイントは維持され、file-backed 実装へ委譲。 |

## 過去の未決事項

当時の見直し案を残します。現在の#31の作業キューやrelease checklistを上書きしません。


```text
- repack / merge / compact は post-Core maintenance phase
- embedded qzt-search-block-v1 は optional future work
- competitive benchmark は QZT vs raw zstd と、feature-gated な ripgrep / SQLite FTS5 correctness hooks まで実装済み
- Tantivy / Lucene / seekable zstd / split zstd frames との比較は post-v0.1 product validation の次課題
```
