# Phase8: Dictionaries, resource limits, and Reader Core completion

[English](Phase8.md)

## 目的

Reader Core の残り義務を満たし、悪意ある file に対する parser / decoder hardening を行います。

## Minimum MVP

```text
- embedded Dictionary Block parsing
- dictionary-assisted zstd decode
- missing / duplicate / checksum dictionary rejection
- resource limit enforcement
```

## Goal MVP

```text
- unknown optional block は安全に無視
- unknown required block は拒否
- index / dictionary / chunk / decode limits
- Reader Core complete
```

## TDD / 実装

dictionary fixture、checksum mismatch、duplicate dictionary id、missing dictionary、resource limit failure、unknown block handling を test します。

## 完了条件

Reader Core の required behavior が揃い、corrupt / expensive input を panic なしに拒否できること。

## 状態

Complete。2026-06-07 に完了済みです。
