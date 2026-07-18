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

## Result

The authoritative record will be copied from
[production run 1](raw/2026-07-partial-decompression/production-run-1.log)
after the production command succeeds.

Peak memory is measured in a separate process with the
`partial_decompression_probe` example after corpus generation and packing have
finished. On macOS, `/usr/bin/time -l` reports this as `maximum resident set size`.

## Reproduce

```sh
make bench-partial-decompression
```

Override `QZT_PARTIAL_BENCH_CORPUS_BYTES` for a smaller local smoke. The default
is 1 GiB. Timing varies with hardware and system load, so the test gates exact
restoration and bounded structural work rather than a machine-specific latency.
