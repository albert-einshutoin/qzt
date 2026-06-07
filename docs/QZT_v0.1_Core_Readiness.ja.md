# QZT v0.1 Core Readiness

[English](QZT_v0.1_Core_Readiness.md)

日付: 2026-06-07

## 目的

この文書は、参照実装が Phase9 の Core release gate を満たしているかを記録するものです。

## Core-ready の範囲

```text
- no-dictionary Writer Core
- embedded-dictionary Reader Core
- sparse Chunk Table による line access
- pack/info/export/range/line/verify CLI
- quick / normal / deep verify
```

## Core-ready に含めない範囲

```text
- dictionary-emitting writer CLI
- Dense Line Index
- Document Index
- Search Extension
- sidecar indexes
- repack / merge / compact
```

これらは後続 Phase または拡張機能です。Core ready の判定には含めません。

## Fixture corpus

ローカルテストは以下を含みます。

```text
- empty input
- LF の 1 行 / 複数行
- CRLF / mixed newline
- lone CR
- 日本語と emoji を含む UTF-8
- invalid UTF-8 の writer rejection
- 長い 1 行 chunking
- 小さい chunk size
- 壊れた fixed structure
- 壊れた deterministic CBOR
- 壊れた Chunk Table
- 壊れた compressed chunk
- embedded dictionary reader fixture
- unknown optional / required Index Root block
- resource limit failure
```

## Conformance

`tests/phase9_hardening.rs` の `CORE_CONFORMANCE_MAP` が、仕様書の Core conformance tests 1-77 をローカルテストへ対応づけています。

Phase9 の検証コマンド:

```text
cargo test --test phase9_cli_core
cargo test --test phase9_hardening
make check
```

## Fuzz smoke

Phase9 では deterministic な malformed-input smoke harness を使います。空、truncated、repeated-byte、bit-flipped、valid、pseudo-random input を open し、可能なら quick/normal/deep verify まで走らせ、panic しないことを確認します。

長時間 fuzz は将来の security hardening で `cargo-fuzz` を導入する余地があります。

## Performance smoke

最新のローカル smoke output:

```text
phase5_pack_smoke bytes=65536 elapsed_ms=6.284 throughput_mib_s=9.945
phase7_bench pack_mib_s=10.909 export_mib_s=36.692 range_mib_s=17.517 line_us=5.250
```

これは smoke baseline であり、SLA ではありません。

## Release gate result

`make check` が通り、Phase9 が `tasks/status.md` で Complete なら、QZT v0.1 Core は release candidate とみなします。
