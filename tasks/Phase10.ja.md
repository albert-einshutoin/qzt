# Phase10: Dense Line Index, Document Index, memory profile, and maintenance command scoping

[English](Phase10.md)

## 目的

Core に影響しない optional index を追加し、memory profile と maintenance command の scope を明確にします。

## Minimum MVP

```text
- Dense Line Index optional block
- sparse vs dense benchmark
- memory profile flag
```

## Goal MVP

```text
- Document Index
- deep verify で optional index consistency check
- qzt repack / merge / compact の scope decision
- memory profile fixture
```

## TDD / 実装

Dense Line Index encode/decode、count mismatch、deep verify disagreement、Document Index ranges、memory profile metadata を test します。

## 完了条件

optional index が Core reader を壊さず、cache として verify 可能であること。

## 状態

Complete。2026-06-07 に完了済みです。
