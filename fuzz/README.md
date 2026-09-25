# QZT fuzz smoke

The weekly and manually dispatched [fuzz-smoke workflow](../.github/workflows/fuzz-smoke.yml)
runs three independent Linux/AddressSanitizer jobs. `open_verify` keeps the Core
pack/open/verify path. `qzi_search` exercises in-memory and lazy file-backed QZI
open/search. `dli_decode` calls the DLI decoder directly with a small, real
chunk plan. `cargo test --manifest-path fuzz/Cargo.toml --test seed_replay
--locked` replays the checked-in QZI/DLI seeds through those exact harness
functions and asserts expected acceptance or rejection at the search/decode
stage. Existing `phase10_dense_line_index`, `phase19_search_budgets`,
`phase19_resource_governance`, and `phase13_sidecar` tests supply the detailed
Reader and counted-I/O assertions; fuzzing does not replace them.

## Input and reachability

The seed files in `seeds/<target>/` are tracked. The workflow copies them to
ignored `corpus/<target>/` before starting libFuzzer. Its `READ`/`INITED` lines
and `seeds.txt`/`seed-hashes.txt` show that the initial corpus was loaded.
Generated corpus files and crash inputs remain under ignored `corpus/` and
`artifacts/`; only reviewed regression inputs should be promoted to `seeds/`.

`qzi_search` uses at most 256 input bytes. Bytes 0–5 select the mode, source,
query, product budget, section mutation, and cap value. Bit 7 of byte 0 feeds
the remaining raw bytes directly to both product QZI open paths. Otherwise a
cached QZT/QZI pair is built from one of eight fixed UTF-8 sources (at most 28
bytes, 16-byte chunks). Bit 0 selects token or n-gram; bit 1 selects legacy v1
or compact v2. The section mutation changes a fixed-width granule, term, or
posting field and recalculates that section's checksum using an
`internal-testing`-only helper. The QZT source binding and all production
checksum checks remain active. The two open/search paths run independently:
file-backed open may accept a malformed granule that its search rejects after
fetching the record. The default selector uses a present query and enough
budget to reach posting and granule processing. Valid, uncapped generated QZI
must open and search successfully on both paths, produce the same hits and
stop/completeness state, and each hit must match bytes in the source. Arbitrary
corruption may return a normal `Err` on either path. The harness does not
claim index coverage from a sidecar's `complete` declaration.

`dli_decode` also caps input at 256 bytes. Byte 0 bit 7 selects raw encoded DLI
bytes after the four-byte header. Otherwise byte 1 selects one of the same
small source/chunk fixtures, byte 2 selects a bounded encoding mutation, and
byte 3 selects an allocation budget of zero, one below exact, exact, one above,
or 256 bytes. Bit 6 makes the first chunk declare `u64::MAX` lines; bit 5
declares a `u64::MAX` uncompressed size for checked delta overflow. Neither
allocates such a source. Valid input with sufficient budget must decode and
pass a separate source-byte `verify_chunk` check. Corrupt inputs are allowed to
return `Err`; a structurally valid DLI is not automatically assumed to match
the original text.

| Tracked seed families | Regression basis and observed product stage |
| --- | --- |
| `huge_span_*`, `empty_span_*`, `outside_span_*`, `overflow_offset_*`, `zero_length_*` | #287: checksum-valid v1/v2 granule is rejected by memory open and by file-backed search after lazy open, with a present token/ngram query. |
| `posting_mutation_*`, `term_mutation_*`, `budget_*`, `token_and_boundary`, `ngram_overlap` | #289/#291: posting/term decoding, query and posting work caps, logical and physical decode caps, candidate and result caps, AND token matching and overlapping `aa` matches. Counted-I/O exact boundaries remain in `phase19_search_budgets` and `phase19_resource_governance`. |
| `valid_*` QZI | Empty source, LF, CRLF, UTF-8, multi-chunk, continuation, token/ngram and v1/v2 normal search. |
| `impossible_chunk_lines`, `huge_entries`, `huge_offsets`, `raw_*`, `truncated`, `noncanonical`, `zero_delta`, `delta_overflow`, `offset_out_of_range`, `offset_mutation`, `trailing` | #288: untrusted DLI counts, varint/delta, range and trailing-data rejection before count-driven allocation. Reader-level malformed container coverage remains in `phase10_dense_line_index`. |
| `budget_below`, `budget_exact`, `budget_above`, `valid_*` DLI | #288: cumulative entry+offset allocation boundary across two chunks; valid continuation with zero newly-started lines is accepted. |

## Limits and execution

Each job uses `cargo-fuzz 0.13.2` and the checked-in `fuzz/Cargo.lock`. The
workflow records the exact nightly compiler version and checkout SHA. The
product-facing QZI limits are 4 KiB manifest, 8 KiB terms/postings, 128 terms
and posting IDs, 32 granules, and 4 KiB per posting list/query. Search limits
are at most 64 query bytes, 16 keys, 4 KiB posting bytes, 128 posting IDs,
512 posting work, 32 candidates, 256 logical and physical decoded bytes,
32 decoded chunks, and 32 hits. Each selector reduces one budget to
0/1/2/4/8/16/32/64, or the checked-in alpha/beta fixture's exact budget
minus one, exact, or plus one. The result-span seeds include caps 0/1/2/3/4
for three actual hits. DLI uses at most 256 bytes for its allocation budget.

Each libFuzzer run has `-max_total_time=60`, `-timeout=10`, `-max_len=4096`
for Core or `256` for QZI/DLI, `-rss_limit_mb=1024`,
`-malloc_limit_mb=128`, and `-seed=294`. The Linux `timeout` wrapper allows 90
seconds plus 15 seconds to terminate, leaving room for log and artifact upload
inside the 15-minute job limit. Reaching libFuzzer's total time normally exits
zero; a per-input timeout, crash, sanitizer finding, allocation failure, RSS
limit or outer deadline exits nonzero. RSS is checked about once per second,
not a precise peak-memory guarantee. These process limits are independent of
QZT's resource limits. See the [libFuzzer option reference](https://llvm.org/docs/LibFuzzer.html#options).

Run locally after `cargo +nightly fuzz build --sanitizer address qzi_search`
(substitute another target) and copying seeds:

```sh
mkdir -p fuzz/corpus/qzi_search fuzz/artifacts/qzi_search
cp fuzz/seeds/qzi_search/* fuzz/corpus/qzi_search/
cargo +nightly fuzz run --sanitizer address qzi_search fuzz/corpus/qzi_search -- \
  -max_total_time=60 -timeout=10 -max_len=256 -rss_limit_mb=1024 \
  -malloc_limit_mb=128 -seed=294 -artifact_prefix=fuzz/artifacts/qzi_search/
```

On failure, the workflow uploads an artifact named `fuzz-<target>-<commit>`
for seven days. It contains the checked-out commit, compiler and cargo-fuzz
versions, sanitizer and arguments, seed hashes, log, exit status when the run
started, and any saved crash/timeout input. The artifact still has setup
records if installation or build fails before an input is saved. Download it,
then replay the input with the recorded toolchain and options:

```sh
cargo +nightly fuzz run --sanitizer address qzi_search fuzz/artifacts/qzi_search/crash-... -- -runs=1
cargo +nightly fuzz run --sanitizer address qzi_search fuzz/artifacts/qzi_search/crash-... -- -minimize_crash=1 -runs=10000
```

After minimizing, add a deterministic test asserting the affected product
contract and a named seed only after reviewing the input. A clean 60-second
smoke proves bounded exploration of these entry points, not absence of all
bugs or completeness of any untrusted QZI index. The known macOS ASan runtime
initialization failure is separate from the Linux ASan result.
