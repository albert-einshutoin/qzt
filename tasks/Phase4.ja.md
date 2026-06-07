# Phase4: UTF-8 chunker and sparse Chunk Table writer

[English](Phase4.md)

## 目的

元 UTF-8 text を安全に chunk に分割し、sparse line metadata を持つ Chunk Plan を作ります。

## Minimum MVP

```text
- deterministic Chunk Plan
- logical offsets
- first_line / line_count
- invalid UTF-8 rejection
```

## Goal MVP

```text
- UTF-8 boundary safe
- CRLF boundary safe
- line-preferred split
- starts_with_line_continuation flag
```

## TDD / 実装

ASCII、empty、multi-byte UTF-8、日本語/emoji、CRLF、long line、tiny chunk size の boundary test を先に書きます。

## 完了条件

Chunk Plan が contiguous logical range を作り、line_count と continuation flag が仕様通りになること。

## 状態

Complete。2026-06-07 に完了済みです。
