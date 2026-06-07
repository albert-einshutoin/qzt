# Phase5: No-dictionary zstd writer and finish

[English](Phase5.md)

## 目的

no-dictionary Writer Core を実装し、`export(pack(input)) == input` を成立させます。

## Minimum MVP

```text
- UTF-8 input を independent zstd frames に encode
- Chunk Table record を書く
- Header を finish 時に patch
- export_all で復元
```

## Goal MVP

```text
- compressed / uncompressed BLAKE3 checksums
- Footer Payload の fixed-point final_file_size
- container_checksum
- writer option が Metadata に反映される
- pack smoke benchmark
```

## TDD / 実装

empty、ASCII、日本語/emoji、CRLF、long line の pack/export equality を先に確認します。checksum と header offset も test します。

## 完了条件

no-dictionary output が valid QZT として開け、exact export できること。

## 状態

Complete。2026-06-07 に完了済みです。
