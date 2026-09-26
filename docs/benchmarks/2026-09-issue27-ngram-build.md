# N-gram sidecar build RSS: one posting representation change (#27)

This is a local development-code measurement, not a published pre.3 binary
benchmark or a production memory bound. The starting main commit was
`fbef491d6f01d7ab5711dbe81eb01cea70168f62`; the measured product change
was committed as `c2f2e283be35a513b93b18ddd99e85c2cf7b5448`. The
primary measure is the
peak resident set size of an independent `qzt sidecar-rebuild` process; the
secondary measure is the parent-observed wall time from process launch through
exit, including QZT open, index construction, serialization, atomic output,
and sync. The plan and adoption rule were recorded on [#27](https://github.com/albert-einshutoin/qzt/issues/27#issuecomment-5846375386)
before changing the builder.

## Input and environment

The existing `cli_cost_probe` C2Logs generator used seed 295, a 100 MiB base,
and its unique marker. The resulting UTF-8 text is 104,857,624 bytes, 806,580
LF lines, SHA-256 `0313a76087036f2a1784b49cdc533282166e5c0d3bb0c1bc4fa47676a30aadcb`.
The same 401-chunk QZT was used for every before/after run: chunk and maximum
chunk sizes 262,144 bytes, 5,764,482 bytes on disk, SHA-256
`065b619f8ca14f1d39e692b9d819fca45db2967c48ad57aa12207bb4bb3b810b`.
Deep verification and a full export byte comparison passed before timing.
N-gram width was 3 with otherwise default options.

Runs used macOS 27.0 (26A428), Apple M4, 32 GiB RAM, internal APFS storage,
Rust/Cargo 1.96.0, default release features, no `RUSTFLAGS` or Cargo profile
overrides, and `Cargo.lock` SHA-256
`f9a502b004998279454cb5cc6aaa76318dec7a9bff784fc561d23baf64a67192`.
The frozen old binary was SHA-256
`90c7fdb2c877c1cc79914417c62cc974179f1de3f15d9eec4efa9033c2c5b1e3`;
the final changed binary was
`04b51aa42fc31499e8e724bb090b8406fe97b097699d59923f81ce43afcd4f9a`.
Both were built by `cargo build --release --locked --bin qzt` from their
respective trees. Rebuilding from the committed changed source returned the
same binary SHA-256 as the final measured binary. The OS file cache was
uncontrolled; no cold-cache result is
claimed. The synthetic repetitive corpus is not representative of all logs.

## Profile and change

A separate old-binary 100 MiB run took 19.24 seconds and reached 3,175,296
KiB in 250 ms RSS sampling. Of 677 CPU samples, 671 were in
`RawNgramIndex::build_from_file`; 308 were under the posting-map insert in
`emit_line_granule`, including 289 in `BTreeMap::insert`, and 203 were in
byte comparison in the same emitter. A 20 MiB old-binary `heap -s` snapshot
at two seconds counted about 2.35 million live malloc nodes, including about
2.01 million in the 112-byte class and 0.33 million in the 224-byte class.
The heap tool did not attribute these nodes to Rust allocation sites. A
separate 20 MiB run with `MallocStackLogging=1` allowed `malloc_history
-allBySize` to attribute its two largest allocation groups to
`emit_line_granule` → `BTreeMap::insert` → `insert_recursing`: 1,195,545 calls
for 133,901,040 bytes, and 195,228 calls for 43,731,072 bytes. That
instrumented snapshot is **not** included in the adoption timing or RSS
values. It emitted one stack-acquisition warning, so only the recorded stacks
are interpreted. An earlier attempt with `xctrace Allocations` suspended the
child without completing; it is **not** counted as an allocation profile.
The CPU and allocation evidence, with the source's per-key `BTreeSet<u64>`
posting storage, identified the posting representation as the candidate;
the exact share of peak RSS by allocator class was not measured.

The sole product change stores each posting list in a `Vec<u64>` instead of a
`BTreeSet<u64>`. Granules are emitted in ascending ID order. A last-ID check
keeps one posting for each key per granule, so each list remains strictly
increasing without a tree node per posting. The sorted outer `BTreeMap`
still determines term order. Query, tokenization, QZI format, and limits were
not changed. The index and serialized QZI still grow with vocabulary and
posting volume; this is not a fixed RSS cap.

## Paired results

Each binary had one unmeasured warmup. Six measured processes then ran in the
frozen order before/after/after/before/before/after, each to a new QZI output.
Each had a 120-second timeout; no run timed out, failed, or was excluded.
macOS `/usr/bin/time -l` reported **child** maximum resident set size in
bytes. Python's monotonic clock measured the complete command wall time.
The final measurements use the rebuilt binary after a source comment was
added. The preceding full series, using changed-binary SHA-256
`47ed089bb5ac57d15dcabbae61cecc424f2ce64a0affd4993617b622da8c3dbc`,
is retained separately in
[paired-results-initial.json](raw/2026-09-issue27-ngram-build/paired-results-initial.json)
and is not mixed into the final medians. Only the comment changed the product
source between these series; the binary hash changed, so the complete paired
measurement was repeated.

| Binary | Measured peak RSS, bytes | Median peak RSS | Measured wall, s | Median wall |
| --- | --- | ---: | --- | ---: |
| Before | 3,172,859,904; 3,268,214,784; 3,139,682,304 | 3,172,859,904 (3,025.9 MiB) | 20.0617; 20.3519; 19.9273 | 20.0617 s |
| After | 1,308,737,536; 1,306,198,016; 1,305,772,032 | 1,306,198,016 (1,245.7 MiB) | 11.8911; 11.4479; 11.0508 | 11.4479 s |

The observed median RSS fell **58.8%** and wall time fell **42.9%**. Both
exceeded the predeclared adoption rule of at least 20% lower RSS and no more
than 10% slower wall time. Every warmup and measured run produced the same
116,370,474-byte QZI, SHA-256
`b032e7f3781a8830e4f9957cc3047e0eaab215bacb96778d24f0be3e8122cce9`.
The sample is three runs per binary on one host, not a cross-platform or
population estimate. The report retains every observation and stderr in
[paired-results-final.json](raw/2026-09-issue27-ngram-build/paired-results-final.json).

## Compatibility and replay

A new small test checks repeated `aaa` n-grams in successive lines against
one sorted posting per line. It passed on the old and changed code. The
targeted `phase12_ngram_planner` and `phase13_sidecar` suites passed after the
change (11 and 31 tests). `phase19_resource_governance`,
`phase19_search_budgets`, and `phase36_search_json` also passed (7, 5, and
12 tests), covering zero hits, verified hits, capped partial results, hard
posting limits, and stop/incomplete reason output. Eight extra CLI sidecars
compared before/after bytes for empty input; source lines shorter than `n`;
repeated LF; CRLF without final newline; Unicode; a continuation line; a
diverse synthetic corpus; and token mode. The diverse corpus was generated by
Python `random.Random(27)` over seven ASCII/CJK words and numeric fields,
4,285 bytes, SHA-256
`f92382e4023d4b28894090d836030df82bf753db2f50600bc7f0f68b029f304c`.
All eight QZI outputs were byte-identical and opened; their sources passed
Deep verify and exact export comparison. The repeated case returned
source-verified search hits. The fixture hashes are in
[fixture-results.json](raw/2026-09-issue27-ngram-build/fixture-results.json).

To replay, build and freeze the old main executable, then generate the source
with `cargo build --release --locked --bin qzt --example cli_cost_probe` and
`target/release/examples/cli_cost_probe generate <source.txt> 104857600 295`.
Pack it once with `qzt pack <source.txt> -o <source.qzt> --chunk-size 262144
--max-chunk-size 262144`; Deep verify and compare a full export to the source.
Build and freeze the changed executable with the same Cargo command. Put both
binaries and the single QZT under the root named in the recorded
[run-paired.py](raw/2026-09-issue27-ngram-build/run-paired.py), or change its
`root` constant for a fresh scratch directory. Then run that script on macOS;
it rejects preexisting output names and writes every result incrementally.
The recorded [plan](raw/2026-09-issue27-ngram-build/plan.json),
[CPU sample](raw/2026-09-issue27-ngram-build/before-cpu.sample.txt),
[RSS timeline](raw/2026-09-issue27-ngram-build/before-cpu-summary.json),
and [heap snapshot](raw/2026-09-issue27-ngram-build/before-heap20.txt)
plus [allocation stacks](raw/2026-09-issue27-ngram-build/before-stacklog-by_size.txt)
preserve the profiling limits as well as the observed data. Trailing spaces in
the profiler text were removed for the repository copy; numeric content was
unchanged. The
[fixture script](raw/2026-09-issue27-ngram-build/check-fixtures.py) records
the extra sidecar comparisons and likewise uses the named scratch root.
