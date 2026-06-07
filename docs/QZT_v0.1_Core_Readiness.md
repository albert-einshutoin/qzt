# QZT v0.1 Core Readiness

Date: 2026-06-07

## Scope

This note records the Phase9 Core release gate for the reference implementation.

Core-ready scope:

```text
- no-dictionary Writer Core
- embedded-dictionary Reader Core
- sparse Chunk Table line access
- pack/info/export/range/line/verify CLI commands
- quick/normal/deep verification
```

Out of Core-ready scope:

```text
- dictionary-emitting writer CLI
- Dense Line Index
- Document Index
- Search Extension
- sidecar indexes
- repack, merge, compact maintenance commands
```

## Fixture Corpus

The local test corpus covers:

```text
- empty input
- one-line and multi-line LF input
- CRLF and mixed newline input
- lone CR as ordinary data
- Japanese and emoji UTF-8
- invalid UTF-8 writer rejection
- long single-line chunking
- tiny chunk sizes
- corrupted fixed structures
- corrupted deterministic CBOR structures
- corrupted Chunk Table structures
- corrupted compressed chunks
- embedded dictionary reader fixtures
- unknown optional and required Index Root blocks
- resource-limit failures
```

## Conformance

`tests/phase9_hardening.rs` contains `CORE_CONFORMANCE_MAP`, which maps Core conformance tests 1-77 from `docs/QZT_v0.1_Core_Spec.md` to local test evidence or marks Dense Line Index cases as not applicable until Phase10.

Phase9 verification commands:

```text
cargo test --test phase9_cli_core
cargo test --test phase9_hardening
make check
```

## Fuzz Smoke

Phase9 uses a deterministic malformed-input smoke harness in `tests/phase9_hardening.rs`.

The harness:

```text
- opens empty, truncated, repeated-byte, bit-flipped, valid, and deterministic pseudo-random byte inputs
- runs quick, normal, and deep verify when open succeeds
- asserts that open/verify do not panic
```

This is the Phase9 equivalent fuzz smoke. A later security hardening phase may add `cargo-fuzz` for longer-running coverage.

## Performance Smoke

Latest local smoke output:

```text
phase5_pack_smoke bytes=65536 elapsed_ms=6.284 throughput_mib_s=9.945
phase7_bench pack_mib_s=10.909 export_mib_s=36.692 range_mib_s=17.517 line_us=5.250
```

These numbers are smoke baselines only. They are not release promises.

## Release Gate Result

QZT v0.1 Core is ready when `make check` passes with Phase9 tests included and `tasks/status.md` marks Phase9 complete.
