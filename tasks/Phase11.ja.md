# Phase11: Search granules and raw token index MVP

[English](Phase11.md)

## 目的

QZT Search Extension の最小 MVP として、raw UTF-8 token search を実装します。

## Minimum MVP

```text
- Search Granules
- raw token dictionary
- sorted postings
- candidate intersection
- original-byte verified hits
```

## Goal MVP

```text
- exact key comparison despite hash collision
- delta-varint postings
- CLI qzt search
- search metrics
- normalized index は Phase11 では拒否
```

## TDD / 実装

granule range、posting order、hash collision、stale candidate verification、multi-token query、CLI metrics を test します。

## 完了条件

token search が candidate だけで match を確定せず、QztReader で original bytes を verify して hit を返すこと。

## 状態

Complete。2026-06-07 に完了済みです。
