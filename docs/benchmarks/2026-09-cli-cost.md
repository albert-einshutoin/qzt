# QZT CLI, open, and sidecar cost (September 2026)

This report records local development-code measurements for #295. It is not a
measurement of the published `v0.1.0-pre.2` binaries or a production SLA.

## Measurement contract fixed before the 100 MiB run

`scripts/cli-cost-benchmark.py` drives the prebuilt `target/release/qzt` and
`target/release/examples/cli_cost_probe` executables. The generator uses the
existing deterministic `CorpusKind::C2Logs` generator with seed 295 and appends
one unique marker. The requested base is 104,857,600 bytes (100 MiB); the
actual source size, lines, and hashes are recorded in the raw manifest. The
text is synthetic, ASCII-heavy, repetitive log data and is unusually
compressible. Both QZT chunk sizes are explicitly 262,144 bytes. The QZI writer
uses its current v2 format and default build limits; n-grams have width 3.

Timing boundaries:

| Measure | Start and end |
| --- | --- |
| CLI search | Parent monotonic clock immediately before each new process launch through child exit and full stdout/stderr collection. Includes startup, argument parsing, file-backed QZT open, QZI open/binding/section checks and dictionary loading, search, JSON serialization and output. Excludes later JSON parsing and byte verification. |
| QZT open | `Instant` around `QztFileReader::open_path` on the real file; includes its structural and index checks. |
| QZI open | Next `Instant` around `QziFileSidecar::open_path`; includes binding, section checks and actual dictionary reads/decode, even when work is repeated internally. |
| API search | `Instant` outside the complete `sidecar.search` call on already-open objects. Each query creates its own chunk decode cache. |
| QZI build | Parent clock around a fresh `sidecar-rebuild` process, including QZT open, index build, serialization, atomic output and sync. `/usr/bin/time -l` observes that child process's maximum resident set size in **bytes** on macOS. Each run writes a new file. |
| QZT pack | Parent clock around a fresh `pack` process, excluding corpus generation and binary compilation. |

These intervals overlap; phase percentiles are never added to derive a CLI
percentile. `metrics.query_time_ms` is an inner search interval and is neither
the API-call wall time nor the CLI wall time. `ReadAt` bytes denote requested
application reads, not physical disk traffic. `physical_decoded_bytes` denotes
uncompressed chunk work.

The eight search conditions are token and n-gram, each with rare
`issue295-needle-unique`, missing `issue295-unseen-absent`, default-capped common
`qzt`, and common `qzt` with explicit `max_results=10` and
`max_candidate_granules=2,000,000`. Other search limits retain defaults:
query bytes 4 KiB, terms 256, posting bytes 128 MiB, posting IDs 10 million,
posting work 20 million, logical and physical decoded bytes 256 MiB, physical
chunks 10,000. The default cases pass no limit flags. The explicit common
case must return 10 source-verified hits and stop at `max_search_results`,
with nonzero physical decode. The default common case stops at
`max_candidate_granules` before hit verification; it is reported separately.
Rare must have one uncapped hit, and the missing string must be absent in the
source, not just absent from QZI. Each CLI hit is checked against source bytes
after the batch; Deep verify and full export byte comparison precede timing.

The run plan is **one batch of 32 measured requests** per case and concurrency
1/8/32, with one unmeasured warmup for each batch. The API probe takes nine
measured calls and one warmup per case. Each QZI kind has three independent
build runs. Individual request timeout is 120 seconds and overall experiment
deadline is 3,600 seconds. There are no silent retries or outlier removals.
Failures, timeouts and mismatches retain their attempt ID, elapsed time,
output and status in `records.jsonl`; success-only percentiles carry counts.
Nearest-rank p50/p95/p99 is used, matching `src/benchmark.rs`; p99 of 32
observations is the maximum and has low statistical precision.

Concurrency is a fixed number of simultaneously outstanding independent CLI
requests, not an arrival-rate or SLA experiment. The parent runs one Python
thread per slot, drains each child's stdout and stderr with `communicate`, and
validates JSON only after the whole batch finishes. Per-request latency starts
when a worker obtains a slot, so it excludes queue wait; batch elapsed and
throughput include the whole batch. The raw log records maximum overlap of
spawned, outstanding processes, which must be checked before claiming 32-way
concurrency. Process startup is cold for every CLI request; the API probe
reuses opened Reader and QZI objects. OS file cache is **uncontrolled** and
may be warm from generation, Deep verification and earlier measurements. No
OS-cache-cold result is claimed. Query chunk decode caches do not survive
between searches.

## Small pilot and method review

The 2 MiB pilot used seed 295, two samples, one warmup, one build run and a
30-second request timeout. All eight correctness cases passed after correcting
the expected result-cap reason to the actual `max_search_results` spelling.
The method review then removed explicit limit flags from default CLI cases and
added spawned-process overlap accounting. A second pilot verified all 24
case/concurrency cells and both independent RSS reads; the 32-slot cells in
that pilot had only two requests and are **not** 32-way evidence. The large
run will require at least 32 requests per cell and observed overlap.

## Environment, raw evidence and results

Pending the frozen-harness run. The raw manifest and request-level log will be
linked here. The report will separate observations from hypotheses and retain
the historical [July v0.1 report](2026-07-v0.1.md) unchanged.
