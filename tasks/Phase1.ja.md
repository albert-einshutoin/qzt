# Phase1: Deterministic CBOR, primitives, and errors

[English](Phase1.md)

## 目的

QZT の CBOR schema と fixed binary parsing の土台を作ります。

## Minimum MVP

```text
- deterministic CBOR encoder / validator
- primitive little-endian helpers
- QztError の基本形
- closed schema helper
```

## Goal MVP

```text
- non-shortest integer、duplicate key、unsorted map、tag/float rejection
- checked arithmetic helper
- property-style round trip tests
- conformance で使える specific error
```

## TDD / 実装

CBOR の canonical rejection test、primitive read/write round trip、overflow error test を先に書きます。parser は corrupt input を通常入力として扱い、panic しないようにします。

## 完了条件

CBOR と primitive tests が通り、後続 Phase が同じ error / primitive helper を使えること。

## 状態

Complete。2026-06-07 に完了済みです。
