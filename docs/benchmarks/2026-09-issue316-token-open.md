# Token QZI file-backed open: one measured hash-pass improvement (#316)

This is a local development-code result, not a measurement of the published
`v0.1.0-pre.3` binary or a cold-cache SLA. Starting `main` and the before
product commit were `eacc627a15dd8e6a11a00ff971934d596c7af7de` (#27 / PR
#315 merge). The harness was committed at `e49442deba49364a023457d71f035200d2127df0`;
the changed product code was measured at
`7eee0e8f85ca28f88a673afd03d6ec989b5b1ac1`. Report/raw-only commits
following this measurement do not change either measured executable. The
[frozen adoption rule](https://github.com/albert-einshutoin/qzt/issues/316#issuecomment-5847212239)
was posted after the pilot and before the product edit.

## Fixed input and measurement boundary

The existing `cli_cost_probe` generated C2Logs seed 295 with a 100 MiB base
plus its unique marker. The 104,857,624-byte source and one QZT/token QZI
pair were built **once** and read by both binaries. A 1,048,600-byte source
from the same seed and its own one-time QZT/token QZI pair were the small
control. Both QZT containers passed Deep verify and byte-exact export. The
source has one occurrence of `issue295-needle-unique` and no occurrence of
`issue295-unseen-absent`. All returned CLI hits were checked against source
bytes and result/coverage/stop contracts. No results were excluded.

| Input | Source | QZT | Token QZI | Terms | Granules | Granule/term/posting sections |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Main | 104,857,624 B | 5,764,482 B | 38,003,559 B | 553,028 | 806,580 | 16,131,608 / 7,188,745 / 14,682,381 B |
| Small | 1,048,600 B | 59,588 B | 406,706 B | 8,176 | 8,067 | 161,348 / 105,596 / 138,937 B |

Both sidecars use `qzt.sidecar.v2`; header plus manifest is 825 bytes.
All source/QZT/QZI and binary SHA-256 values, compiler/OS/CPU/memory/filesystem,
features, profile, flags, query limits and replay command are in the
[environment record](raw/2026-09-issue316-token-open/environment.txt). The
same Rust 1.96.0, `Cargo.lock`, default features and release profile built the
before and after binaries with `cargo build --release --locked --bin qzt
--example cli_cost_probe`.

The probe's separate `Instant` intervals surround QZT file open, then QZI
file open, then one API search on already-open objects. **Each probe sample
starts a new process and opens QZI once.** The independent CLI wall interval
runs from parent launch through exit/output collection and includes startup,
QZT/QZI open, search and JSON. `/usr/bin/time -l` observes child peak RSS in
bytes. These intervals overlap and their medians cannot be added. The final
CLI has `--format json` and otherwise default limits; the API probe passes
the same 10,000 result/candidate limits explicitly. The cache was not reset;
fixture creation, Deep verify, pilot and profiling may have warmed it. A new
process does not imply an OS-cache-cold read.

A four-sample before-only pilot found main QZI open medians 137.396 ms (rare)
and 136.907 ms (missing); the rare range was 133.901–140.266 ms and missing
range 135.045–145.329 ms. One missing CLI observation was 187.538 ms against
a 146.510 ms median and remained in the pilot raw data. The frozen formal
plan was 32 measured observations per binary × input size × query, one
unmeasured warmup, and alternating before/after order per ordinal. Each child
had a 15-second timeout and the run a 900-second deadline. There was no retry
or outlier filtering. This is a targeted serial comparison, not #295's full
concurrency/query matrix.
The main rare before median rose from 137.396 ms in the pilot to 161.867 ms
in the final run, a reminder that cache/system state is uncontrolled; the
final decision uses only the interleaved formal run.

## Profile and selected cause

The instrumented `profile` probe counted 7 QZT `ReadAt` requests for 53,440
bytes and 586 QZI requests for 45,192,312 bytes on the main input. The QZI
file is 38,003,559 bytes; the excess is the 7,188,745-byte term section read
again after checksum verification plus an 8-byte granule count. The small
QZI used 12 requests for 512,310 bytes. These are application-requested bytes,
**not** physical disk I/O. Counts were identical before and after the change.

In a separate 10-second, repeated-open CPU sample, the before call tree
placed roughly 5,001 samples below the QZI term decode site, including two
per-term BLAKE3 paths, and 1,653 below file dictionary validation, including
a third per-term BLAKE3 path. Section checksum verification occupied about
999 samples in that sample. In the after sample, the validation hash branch
is absent while one derived-hash path remains in decode; section checksums
still execute. The sample runs had different total counts and are diagnostic,
not an exact CPU-share or timing comparison. The full traces are
[before](raw/2026-09-issue316-token-open/before.sample.txt) and
[after](raw/2026-09-issue316-token-open/after.sample.txt).

A separate `heap -s` snapshot during each looping profile observed 783,490
before and 783,658 after live malloc nodes, mostly the 16-byte size class.
These asynchronous snapshots include a varying number of loop iterations and
cannot quantify allocations per open or explain the small difference. Their
[raw before](raw/2026-09-issue316-token-open/before.heap.txt) and
[raw after](raw/2026-09-issue316-token-open/after.heap.txt) are retained.
Only trailing whitespace in the heap tool's text output was removed for the
repository copy; its numeric content is unchanged.
Formal child peak RSS below is the regression check. Read and allocation
probes were not used in formal timing.

The one product edit removes redundant per-term hash comparisons for
CompactV2. Its key hash is **derived** from the bytes decoded into the term;
there is no stored v2 hash to authenticate again. LegacyV1 **stores** a hash
supplied by the file; file and in-memory open still compare it with a fresh
hash of the decoded key. Section checksums, source binding, layout and bounds,
term ordering/frequency/posting validation, incremental allocation, and
query-time lazy posting/granule reads stay on their existing paths. We did not
select the repeated term-section read: removing it would require a larger
read/verification refactor for a smaller observed CPU share.

## Paired observations and adoption

The [final raw records](raw/2026-09-issue316-token-open/final/records.jsonl)
contain **all 528** process observations (512 measured and 16 warmups), each
command, status, elapsed time, child RSS, probe phases or CLI JSON/result
verdict, and stdout/stderr. The [manifest](raw/2026-09-issue316-token-open/final/manifest.json)
and [summary](raw/2026-09-issue316-token-open/final/summary.json) preserve
hashes, plan and per-case timing. The [pilot records](raw/2026-09-issue316-token-open/pilot/records.jsonl)
are separate. All 528 final processes exited successfully; all CLI result
checks passed; there were no timeouts or excluded outliers. Values below are
medians with the full observed min–max range in milliseconds.

| Input/query | QZI open before → after | Independent CLI before → after | Open change | CLI change |
| --- | --- | --- | ---: | ---: |
| Main rare | 161.867 (143.418–287.268) → 95.873 (84.915–213.570) | 173.064 (157.144–571.734) → 112.631 (93.943–204.039) | −40.8% | −34.9% |
| Main missing | 150.519 (139.581–281.829) → 88.398 (81.858–260.248) | 162.143 (149.623–407.690) → 100.478 (91.382–128.906) | −41.3% | −38.0% |
| Small rare | 2.125 (2.013–2.377) → 1.255 (1.210–1.399) | 6.643 (6.170–8.923) → 5.694 (5.258–9.464) | −40.9% | −14.3% |
| Small missing | 2.189 (2.056–6.218) → 1.311 (1.209–2.422) | 6.881 (6.125–22.050) → 5.904 (5.294–15.862) | −40.1% | −14.2% |

QZT open medians were 0.132→0.134 ms (main rare), 0.125→0.124 ms
(main missing); open-object API search was 0.092→0.094 ms and
0.023→0.023 ms respectively. The changes to those much smaller intervals
are below useful attribution. The reduction in independent CLI time is
consistent with the QZI open reduction, but overlapping intervals and CLI
startup prevent deriving an exact contribution by subtraction.

| Input/query | Probe peak RSS before → after | CLI peak RSS before → after |
| --- | ---: | ---: |
| Main rare | 73,416,704 → 73,400,320 B | 73,654,272 → 73,695,232 B |
| Main missing | 73,252,864 → 73,220,096 B | 73,465,856 → 73,482,240 B |
| Small rare | 3,424,256 → 3,407,872 B | 3,653,632 → 3,686,400 B |
| Small missing | 3,309,568 → 3,293,184 B | 3,489,792 → 3,522,560 B |

These are medians of 32 individual child peaks, not summed memory or a
controlled system peak. Main CLI RSS rose by at most 0.06% here; small CLI
RSS rose by at most 0.94%. Main QZI open exceeded the predeclared 15%
threshold and CLI rare/missing exceeded 10%. Neither RSS nor small control
breached its regression rule, and the code change is two hash-condition edits.
**Adopt the product change.** The ranges overlap due to system variability;
the paired alternating order and large median gap support this local decision,
not a population confidence interval.

## Validation and limits

Before the edit, the new checksum-consistent LegacyV1 bad-key-hash fixture
passed as a rejection test; it also passed after the edit on both file and
in-memory opens. The related `sidecar::manifest_tests` (23), `phase13_sidecar`
(31), `phase19_resource_governance` (7), `phase19_search_budgets` (5), and
`phase36_search_json` (12) all passed after the edit. These exercise v1/v2,
token/ngram, source binding including a different search Reader, corrupt and
unused sections, malformed counts/ranges, lazy query reads, zero/capped/early
results, hard errors, stop/incomplete reports, and JSON. The v2 frequency
fixture also recomputes the section checksum, so dictionary validation cannot
be bypassed merely by changing the outer checksum. Main/small Deep verify and
byte-exact export passed. Full local, MSRV, package, review and hosted CI
results are recorded on #316 and the PR after completion; they were not part
of the timed run.

The input is synthetic, repetitive and unusually compressible. The host is a
single Apple M4 macOS machine with APFS and uncontrolled cache. Linux,
Windows, real production logs, cold-cache disk work, concurrently running
searches, and the public pre.3 executable were not measured. The unchanged
7.19 MiB term reread and more costly query cases are follow-up candidates
only after a separate profile and safety design; this task does not change
QZI format, public API, CLI defaults, release, tag or crates.io artifacts.
