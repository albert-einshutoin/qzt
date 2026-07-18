# QZT production-scale partial-decompression evidence (2026-07)

This report closes the range-work measurement gap documented in the v0.1
benchmark report. It is reproducible local evidence, not an SLA.

## Question and contract

The probe asks whether a 64 KiB logical range in a deterministic 1 GiB C2 log
corpus can be restored without decoding the whole source. The machine-readable
record distinguishes returned bytes from full intersecting-chunk
`decoded_bytes` and compressed chunk payload `compressed_bytes`.

QZI search is deliberately not executed inside this probe. The retained 100 MB
profile matrix measures QZI rare, missing, and capped-common queries; combining
index construction with this range-only process would hide the partial-read
memory and work boundary.

## Environment

- Commit `c5a79f33f4082d80138c53d26cc389e86bab5d16`
- Mac mini, Apple M4, 32 GiB RAM, arm64
- macOS 26.5 (25F71)
- `rustc 1.96.0`, `cargo 1.96.0`
- Measurements ran serially on an interactive developer machine

The full snapshot is retained in
[environment.txt](raw/2026-07-partial-decompression/environment.txt).

## Results

### Deterministic C2 corpus

All three 1 GiB runs restored the exact requested bytes and recorded identical
structural work. Timing is shown as an observation, not a gate.

| Run | Returned | Decoded | Compressed payload | Chunks | Range time |
| --- | ---: | ---: | ---: | ---: | ---: |
| [1](raw/2026-07-partial-decompression/production-run-1.log) | 65,536 B | 262,085 B | 14,339 B | 1 | 213 µs |
| [2](raw/2026-07-partial-decompression/production-run-2.log) | 65,536 B | 262,085 B | 14,339 B | 1 | 167 µs |
| [3](raw/2026-07-partial-decompression/production-run-3.log) | 65,536 B | 262,085 B | 14,339 B | 1 | 164 µs |

The range decoded 0.0244% of the 1 GiB source. It consumed 0.0243% of the
59,015,504-byte QZT container as compressed chunk payload. In contrast, a
whole-stream decoder must materialize or scan the source prefix needed to
reach the same offset. This proves bounded work for this layout; it does not
claim that every range is faster than every competing format.

### Isolated reader memory

The RSS probe used a separately packed 1 GiB all-zero UTF-8 fixture so corpus
generation and packing were outside the measured process. Default 16 MiB QZT
chunks intentionally provide a conservative one-chunk decode case.

| Source | Returned | Decoded | Compressed payload | maximum resident set size |
| ---: | ---: | ---: | ---: | ---: |
| 1,073,741,824 B | 65,536 B | 16,777,216 B | 530 B | 21,184,512 B (20.20 MiB) |

The authoritative process output is retained in
[rss-probe-run-1.log](raw/2026-07-partial-decompression/rss-probe-run-1.log).
This is one local macOS measurement, not a cross-platform memory ceiling.

Peak memory is measured in a separate process with the
`partial_decompression_probe` example after corpus generation and packing have
finished. On macOS, `/usr/bin/time -l` reports this as `maximum resident set size`.

## Reproduce

```sh
make bench-partial-decompression
```

For an isolated RSS measurement, first pack an existing 1 GiB UTF-8 source,
then run:

```sh
cargo build --release --example partial_decompression_probe
/usr/bin/time -l target/release/examples/partial_decompression_probe \
  source.log source.qzt 805306368 65536
```

Override `QZT_PARTIAL_BENCH_CORPUS_BYTES` for a smaller local smoke. The default
is 1 GiB. Timing varies with hardware and system load, so the test gates exact
restoration and bounded structural work rather than a machine-specific latency.

## Boundaries

- The C2 production probe allocates its generated corpus and packed container
  before timing the range. Use the isolated example, not that process RSS, for
  reader-memory evidence.
- The isolated RSS fixture is highly compressible and exists to isolate memory,
  not to represent production compression ratios.
- QZI search evidence remains the separate 100 MB profile matrix in the
  [v0.1 benchmark report](2026-07-v0.1.md); QZI build-time peak RSS is still
  unmeasured.
- Results do not cover concurrent readers, network-backed `ReadAt`, or every
  possible chunk/range alignment.
