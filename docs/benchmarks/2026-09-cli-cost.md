# QZT CLI, open, and sidecar cost (September 2026)

This report records local development-code measurements for #295. It is not a
measurement of the published `v0.1.0-pre.2` binaries or a production SLA.

## Measurement contract fixed before the final 100 MiB run

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
run therefore required at least 32 requests per cell and observed overlap.
After the Codex review, three small subprocess tests covered an expired
deadline before launch, an in-flight child killed at the overall deadline,
and a failed spawn retained alongside a successful sibling. They run with
`PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests -p
test_cli_cost_benchmark.py -v` and passed before the final full run.

## Environment, raw evidence and results

The frozen harness commit was
`365a303a73224bc62cb4681b3dae9a1f46c74386`; its working tree was clean.
The QZT product code was unchanged from starting main
`917dd903d821b99da0197396958f7cff95e690d8`.
The release-profile binary was built with default features and SHA-256
`cae7d34adb8f9091613f957532aef3106276caaeeba26ab1d15cc3eff70f9205`.
The run used macOS 27.0 on an Apple M4 (10 cores, 32 GiB RAM), with generated
files on the internal APFS SSD and the checkout/binary on an external APFS USB
SSD. Rust was 1.96.0; Rust 1.87 accepted the probe with Xcode's 26.5 SDK.
See the [full environment and commands](raw/2026-09-cli-cost/environment.txt),
[manifest](raw/2026-09-cli-cost/manifest.json),
[every attempt and output](raw/2026-09-cli-cost/records.jsonl),
[recomputed summary](raw/2026-09-cli-cost/summary.json), and
[separate resource sample](raw/2026-09-cli-cost/resource-sample.json).
The first 100 MiB run used commit `079b6ad`; a new Semgrep finding led to
bounded `args_os()` parsing and a second full run at `f4a4966`. Codex's review
of the initial PR HEAD then identified two valid failure-accounting issues:
the overall deadline was not enforced within a concurrent batch, and process
launch failures could discard that batch's results. Both were fixed with
small subprocess-failure tests, piloted, and the **complete third run** at
`365a303` under the same conditions supplies this report's raw data and
percentiles. The QZT product binary and generated source hashes stayed the
same. Earlier runs remain in PR commits `775bc53` and `d4c8c04` for audit;
their samples are not mixed into the final run. Their timing variation is a
reason to avoid strong population or SLA claims.

### Source, correctness and capacity

The actual C2+marker source was **104,857,624 bytes**, 806,580 LF lines,
SHA-256 `0313a76087036f2a1784b49cdc533282166e5c0d3bb0c1bc4fa47676a30aadcb`.
The base C2 generator used seed 295, then one unique ASCII marker was appended;
the source had exactly one rare string and no missing string. Its repetitive
timestamps, labels and vocabulary favor compression; it is not a production-log
sample. QZT had 401 chunks, no DLI or document index. Deep verify succeeded,
full export had the same SHA-256, and both sidecars were opened and searched.
All 8 reference searches, 24 warmups, **768/768 measured CLI requests**, and
80/80 API requests passed their result contracts. Every returned hit's byte
offset and length matched the original source. The QZI completeness declaration
was true but coverage was **not** verified; this distinction is retained in
the outputs. There were no timeouts, process errors, or excluded outliers.

| Completed file set | Logical bytes | Ratio to original |
| --- | ---: | ---: |
| Original text | 104,857,624 | 1.000 |
| QZT | 5,764,482 | 0.055 |
| Token QZI | 38,003,559 | 0.362 |
| N-gram QZI | 116,370,474 | 1.110 |
| QZT + token | 43,768,041 | 0.417 |
| QZT + n-gram | 122,134,956 | 1.165 |
| QZT + both | 160,138,515 | 1.527 |

These are file lengths, including QZI header/manifest, not filesystem allocated
blocks or `metrics.index_size_bytes`. Keeping the original separately adds
104,857,624 bytes to each set; for example, original + QZT + both QZI files
is 264,996,139 bytes (2.527 times the original).

### Pack and independent QZI builds

One new-file `qzt pack` process took **478 ms** and produced the 5,764,482-byte
container. This excludes corpus generation and binary build. Each
`sidecar-rebuild` process wrote a **new**, safely synchronized file. Parent
wall time includes all CLI work; macOS `/usr/bin/time -l` reports the child's
maximum resident set size in **bytes**, converted below to GiB. The parent
harness's memory and prior searches are not included in the child peak.

| Index | Build wall times, s (3 runs) | Child peak RSS, MiB (3 runs) |
| --- | --- | --- |
| Token | 4.646, 4.548, 3.466 | 631.6, 694.1, 693.2 |
| N-gram | 33.035, 23.091, 23.332 | 3,060.4, 3,055.0, 3,147.1 |

The n-gram build is the largest measured one-time cost on this corpus. The
RSS rows are independent process peaks, not sums or estimates derived from
the parent's maximum RSS. This macOS unit is bytes; no Linux/Windows RSS
conversion or result is claimed.

### Open objects and search API

Each phase-probe process opens QZT and then QZI from real files. Across four
processes per index kind, QZT open median was **0.146 ms** for token cases and
**0.136 ms** for n-gram cases; the first process in each group took 0.52 and
0.62 ms respectively. QZI open median was **149.2 ms** for token (range
143.0–163.6 ms) and **63.9 ms** for n-gram (62.6–85.1 ms). QZI open includes
its actual section verification and dictionary loading, even when these
repeat reads. The filesystem cache was uncontrolled and already touched by
verification, so none of these are OS-cache-cold timings.

The API probe then reused those opened objects. Its nine measured calls per
condition had these observed p50 wall times; one warmup was excluded:

| Query | Token API p50 | N-gram API p50 | Result contract |
| --- | ---: | ---: | --- |
| Rare | 0.022 ms | 16.119 ms | 1 verified hit; uncapped |
| Missing | 0.002 ms | 0.006 ms | 0 hit; source string absent |
| Common, default | 13.178 ms | 14.534 ms | 0 hit, `max_candidate_granules`, 0 decode |
| Common, explicit finite | 294.585 ms | 348.110 ms | 10 verified hits, `max_search_results`, nonzero decode |

For the explicit common case, each baseline search reported 806,579 candidate
granules, about 1.3 KiB of logical verification reads and 262,085 bytes of
physical decompression. Rare search returned one hit with 23–24 logical bytes
and 23,254 physical decoded bytes. Missing search decoded zero. The API call
does not serialize CLI JSON. QZI's reported `metrics.query_time_ms` starts
inside search after some checks; it is not the API or CLI wall interval.

### New-process CLI search and concurrency

Each row below contains **32 successful measured requests** and one excluded
warmup. Values are observed nearest-rank p50/p95/p99 in milliseconds and batch
throughput in completed requests/second. `max outstanding` is the maximum
overlap of intervals from child spawn through exit/output collection. It
establishes overlapping invocations, including 32 for the expensive
conditions; a separate `ps` sample confirmed 32 live processes for one token
common run. A fast n-gram query could finish
before all 32 slots filled (24–31 observed) and is not labeled as 32
simultaneous CPU tasks. Full per-request wall times, exit status, output bytes,
JSON and correctness verdicts remain in the raw log.

| Query | Slots | p50 | p95 | p99 | Throughput/s | Max outstanding |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| token rare | 1 | 154.5 | 177.4 | 192.4 | 6.33 | 1 |
| token rare | 8 | 250.7 | 262.3 | 263.0 | 31.57 | 8 |
| token rare | 32 | 800.5 | 880.7 | 881.4 | 33.22 | 32 |
| token missing | 1 | 158.1 | 172.6 | 173.0 | 6.27 | 1 |
| token missing | 8 | 255.2 | 269.3 | 270.8 | 30.51 | 8 |
| token missing | 32 | 1,779.2 | 1,864.0 | 1,891.6 | 16.30 | 32 |
| token common/default | 1 | 170.7 | 191.9 | 195.7 | 5.74 | 1 |
| token common/default | 8 | 275.5 | 293.4 | 293.6 | 28.45 | 8 |
| token common/default | 32 | 1,147.6 | 1,277.7 | 1,278.2 | 24.09 | 32 |
| token common/10 hits | 1 | 464.8 | 495.7 | 599.1 | 2.13 | 1 |
| token common/10 hits | 8 | 5,108.2 | 5,509.6 | 5,510.9 | 1.58 | 8 |
| token common/10 hits | 32 | 23,322.9 | 23,628.6 | 23,674.4 | 1.35 | 32 |
| n-gram rare | 1 | 84.2 | 88.6 | 88.6 | 11.81 | 1 |
| n-gram rare | 8 | 122.4 | 133.3 | 139.3 | 59.94 | 8 |
| n-gram rare | 32 | 285.1 | 363.8 | 374.0 | 71.75 | 32 |
| n-gram missing | 1 | 66.9 | 74.1 | 75.7 | 14.72 | 1 |
| n-gram missing | 8 | 98.1 | 126.0 | 128.2 | 74.86 | 8 |
| n-gram missing | 32 | 204.7 | 291.9 | 312.4 | 81.30 | 24 |
| n-gram common/default | 1 | 81.2 | 85.5 | 94.3 | 12.24 | 1 |
| n-gram common/default | 8 | 119.3 | 128.4 | 139.8 | 64.81 | 8 |
| n-gram common/default | 32 | 317.7 | 387.1 | 396.0 | 72.54 | 31 |
| n-gram common/10 hits | 1 | 362.8 | 469.8 | 554.8 | 2.57 | 1 |
| n-gram common/10 hits | 8 | 4,851.3 | 4,975.8 | 5,072.9 | 1.65 | 8 |
| n-gram common/10 hits | 32 | 18,991.9 | 19,282.6 | 19,299.0 | 1.65 | 32 |

The 1-slot rare/missing CLI time is much larger than the reused-object API
call. Separate open probes show token QZI open alone near the token rare CLI
median, supporting QZI open as a major repeated-request cost. These are
overlapping intervals from different processes, not additive component
percentiles or a full CPU profile. The explicit common case spends hundreds of
milliseconds inside the API call and falls to about 1.35–1.65 completed
requests/s at 8–32 slots while per-request latency grows to about 5–23
seconds. A separate 32-child token-common sample observed 32 live processes,
CPU up to 766.8% of one core summed across children, and a 2,837,296 KiB
maximum **sampled sum**
of child RSS. This is not a measured system-wide memory peak. One isolated
child had one observed thread; no exhaustive per-process thread profile was
collected. The 10 logical CPUs were oversubscribed by 32 processes.

## Interpretation, limits and #27 decision

For this synthetic corpus, the measured high costs are n-gram construction
(23.1–33.0 s and 2.98–3.07 GiB peak), token QZI open in every fresh process
(about 149 ms median), and high-frequency hit verification/planning under
concurrency. The data justify **investigating** #27's index-build memory/time
path and repeated token dictionary open, with a profiler and same-condition
before/after comparison before choosing an optimization. They do not isolate
which allocation or dictionary step dominates, so no specific data-structure
change is prescribed. #27 remains a separate deferred implementation issue.

This is one developer machine, one compressible generated corpus and one
measured batch per condition. OS cache was neither reset nor quantified as
cold; the separate warm API probe reuses opened objects but not decoded-chunk
caches. Each latency percentile is an observed sample statistic, not a stable
population estimate; p99 is the maximum of only 32 requests. There were no
failures to retry; a future run must retain failures rather than drop them.
The 32-slot experiment uses fixed outstanding requests, not an arrival-rate
service model. No Linux/Windows measurement, competitor latency, production
trace or SLA is inferred. Historical [July v0.1 measurements](2026-07-v0.1.md)
remain evidence for their own code and conditions.

## Reproduce from a repository checkout

```sh
cargo build --release --bin qzt --example cli_cost_probe
QZT295_RUN="$(mktemp -d /private/tmp/qzt295.XXXXXX)"
python3 scripts/cli-cost-benchmark.py \
  --binary target/release/qzt --probe target/release/examples/cli_cost_probe \
  --work-dir "$QZT295_RUN/work" --log-dir "$QZT295_RUN/raw" \
  --bytes 104857600 --seed 295 --samples 32 --api-samples 9 \
  --warmup 1 --build-runs 3 --timeout 120 --max-wall-seconds 3600
```

The command needs macOS `/usr/bin/time -l` for build RSS. It writes generated
files outside Git and preserves `manifest.json`, `records.jsonl` and
`summary.json`. The legacy July `make bench-profile` path remains a separate
in-process benchmark. The script and example are repository checkout tools;
they are not promised in the crates.io package.
