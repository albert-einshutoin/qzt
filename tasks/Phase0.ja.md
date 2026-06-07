# Phase0: Project foundation and quality gates

[English](Phase0.md)

## 目的

QZT format 実装前に、再現可能な Rust project foundation を作ります。

## Minimum MVP

```text
- Rust workspace または single crate
- CLI 名 `qzt` の予約
- library module skeleton
- test harness
- format / lint command の文書化
```

## Goal MVP

```text
- fmt / clippy / test をまとめた local quality command
- fixture directory layout
- corruption fixture strategy
- 変更ごとの status.md update rule
```

## TDD / 実装

まず library import、CLI help、fixture directory discovery の smoke test を用意します。その後 Cargo project、library skeleton、CLI target、fixtures、Makefile を追加します。

## 完了条件

`make check` が通り、README または status に実行コマンドが記録されていること。

## 状態

Complete。2026-06-07 に完了済みです。
