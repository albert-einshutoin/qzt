# Phase3: Metadata, Footer Payload, Index Root, and Chunk Table skeleton

[English](Phase3.md)

## 目的

zstd chunk をまだ持たない empty container を、structural verifier で開けるところまで作ります。

## Minimum MVP

```text
- Metadata CBOR
- Footer Payload CBOR
- Index Root CBOR
- empty Chunk Table
- empty container writer / opener
```

## Goal MVP

```text
- block ref checksum validation
- source consistency validation
- unknown field rejection
- chunk_count / chunk_table_size validation
```

## TDD / 実装

empty source fixture、metadata/index root mismatch、footer checksum mismatch、unknown field rejection、chunk table size mismatch を test します。

## 完了条件

empty QZT skeleton が write/open でき、壊れた CBOR / refs を拒否できること。

## 状態

Complete。2026-06-07 に完了済みです。
