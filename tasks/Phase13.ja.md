# Phase13: Search sidecar and high-performance search goal MVP

[English](Phase13.md)

## 目的

`.qzi` sidecar を実装し、Search Extension の high-performance search goal MVP を成立させます。

## Minimum MVP

```text
- QZI sidecar header
- deterministic manifest
- section offset/size/checksum validation
- source container id / checksum validation
```

## Goal MVP

```text
- token/ngram sidecar lookup
- sidecar rebuild CLI
- search --sidecar CLI
- common-term cap
- rare-term candidate-only decode evidence
```

## TDD / 実装

wrong source id、wrong checksum、sidecar lookup parity、Core fallback、rare/common query behavior、CLI rebuild/search を test します。

## 完了条件

sidecar が hot / rebuildable index として機能し、QZT Core fallback を壊さないこと。

## 状態

Complete。2026-06-07 に完了済みです。
