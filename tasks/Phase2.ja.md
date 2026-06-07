# Phase2: Header, Footer Trailer, and physical ranges

[English](Phase2.md)

## 目的

QZT の fixed-size binary structures と physical range model を実装します。

## Minimum MVP

```text
- 128-byte Header encode/decode
- 64-byte Footer Trailer encode/decode
- version / magic / flags validation
- half-open physical range helper
```

## Goal MVP

```text
- index_hint_offset を authoritative にしない
- header/footer reserved bytes を検査
- range overlap / bounds / overflow rejection
```

## TDD / 実装

exact layout round trip、magic/version corruption、too-small file、reserved range overlap、offset+size overflow のテストを先に置きます。

## 完了条件

fixed structure tests と physical range corruption tests が通ること。

## 状態

Complete。2026-06-07 に完了済みです。
