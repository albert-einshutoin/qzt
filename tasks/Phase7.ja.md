# Phase7: Sparse line index, range reads, and CLI access

[English](Phase7.md)

## 目的

QZT の主要価値である partial access を user-facing API と CLI にします。

## Minimum MVP

```text
- read_range
- read_line_raw
- byte range CLI
- line CLI
```

## Goal MVP

```text
- chunk boundary をまたぐ range
- line continuation をまたぐ line read
- text range UTF-8 boundary validation
- intermediate benchmark
```

## TDD / 実装

single chunk range、multi chunk range、zero length、overflow、first/last line、spanning line、CLI smoke を test します。

## 完了条件

ユーザーが `qzt range` と `qzt line` で original bytes を正しく取り出せること。

## 状態

Complete。2026-06-07 に完了済みです。
