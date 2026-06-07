# QZT

[English](README.md)

QZT は、大きなテキストを「冷たい証拠コンテナ」として保存するためのバイナリフォーマットです。このリポジトリは Rust による参照実装です。

QZT の目的は、zstd より良い圧縮アルゴリズムを作ることではありません。元テキストを独立した zstd chunk に分け、検証可能なメタデータ、Chunk Table、Footer、検索 sidecar を組み合わせることで、必要な範囲だけを復元し、証拠位置へ戻れるようにすることです。

## 現在の位置づけ

```text
- QZT v0.1 Core: release candidate
- Search Extension / QZI sidecar: technical preview
- Product status: 実験的な参照実装
```

外部に出すときは、production-ready ではなく `v0.1 technical preview` として扱うのが妥当です。

## ローカル品質ゲート

```sh
make check
```

このコマンドは以下を実行します。

```text
- cargo fmt --all -- --check
- cargo clippy --all-targets --all-features -- -D warnings
- cargo test --all-targets --all-features
```

## 主な CLI

```sh
qzt pack input.txt -o output.qzt
qzt info output.qzt
qzt export output.qzt -o restored.txt
qzt range output.qzt --bytes 0:1024
qzt range output.qzt --lines 1:10
qzt line output.qzt 1
qzt verify output.qzt --deep
qzt sidecar-rebuild output.qzt -o output.qzt.qzi
qzt search output.qzt "error" --sidecar output.qzt.qzi
```

## ドキュメント

- 仕様要約: [docs/QZT_v0.1_Core_Spec.ja.md](docs/QZT_v0.1_Core_Spec.ja.md)
- Core readiness: [docs/QZT_v0.1_Core_Readiness.ja.md](docs/QZT_v0.1_Core_Readiness.ja.md)
- Release hardening: [docs/QZT_v0.1_Release_Hardening.ja.md](docs/QZT_v0.1_Release_Hardening.ja.md)
- プロダクト反論: [docs/QZT_v0.1_Product_Counterargument.ja.md](docs/QZT_v0.1_Product_Counterargument.ja.md)
- 実装 Phase: [tasks/README.ja.md](tasks/README.ja.md)
- 進捗: [tasks/status.ja.md](tasks/status.ja.md)

## Phase 計画

実装は [tasks/Phase0.md](tasks/Phase0.md) から [tasks/Phase13.md](tasks/Phase13.md) まで進みます。日本語版は同じディレクトリの `*.ja.md` にあります。

進捗は [tasks/status.md](tasks/status.md) と [tasks/status.ja.md](tasks/status.ja.md) で管理します。

## プロダクト批評

現在の仕様と Phase 計画への意図的に厳しい反論は [docs/QZT_v0.1_Product_Counterargument.ja.md](docs/QZT_v0.1_Product_Counterargument.ja.md) にまとめています。
