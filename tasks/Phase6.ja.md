# Phase6: Reader open/info/export and verification levels

[English](Phase6.md)

## 目的

QztReader を実装し、open/info/export と quick/normal/deep verify を分離します。

## Minimum MVP

```text
- QztReader::open
- info
- export_all
- quick verify
```

## Goal MVP

```text
- normal verify
- deep verify
- compressed / uncompressed checksum mismatch detection
- container_checksum detection
- decode output size limit
```

## TDD / 実装

valid Phase5 container、corrupt compressed bytes、checksum mismatch、deep verify decoded byte count を test します。

## 完了条件

quick は decompression なし、normal/deep はより強い integrity check を行い、壊れた container を specific error で拒否できること。

## 状態

Complete。2026-06-07 に完了済みです。
