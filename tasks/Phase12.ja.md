# Phase12: N-gram index, planner, and benchmark reporting

[English](Phase12.md)

## 目的

raw n-gram search、query planner、benchmark reporting を実装します。

## Minimum MVP

```text
- raw Unicode-scalar n-gram index
- substring verification
- missing-key behavior
- query metrics
```

## Goal MVP

```text
- rarest-first planner
- high document-frequency term handling
- deterministic skip metadata
- benchmark report completeness
```

## TDD / 実装

boundary-crossing substring、missing key、incomplete index、planner rarest-first、high-DF avoidance、skip data、CLI ngram search を test します。

## 完了条件

n-gram search が original-byte substring として verify され、performance claim に必要な metrics を出せること。

## 状態

Complete。2026-06-07 に完了済みです。
