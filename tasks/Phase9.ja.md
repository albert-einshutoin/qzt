# Phase9: Core conformance hardening and release readiness

[English](Phase9.md)

## 目的

QZT v0.1 Core release candidate にするため、conformance、CLI、fuzz smoke、readiness note を揃えます。

## Minimum MVP

```text
- Core conformance map
- Core CLI integration tests
- malformed open/verify fuzz smoke
- readiness note
```

## Goal MVP

```text
- all Core conformance tests pass
- benchmark smoke
- public API docs polish
- Core release readiness notes
```

## TDD / 実装

仕様の MUST を test name に対応づけ、不足 fixture を埋めます。CLI pack/info/export/range/line/verify を integration test します。

## 完了条件

`make check` が通り、Core release readiness note が存在し、`tasks/status.md` が Core ready を示すこと。

## 状態

Complete。2026-06-07 に完了済みです。
