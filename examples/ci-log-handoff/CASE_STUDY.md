# A failed release build, read through QZT

This is our own small CI investigation, not evidence of external adoption. We
used the **published** `v0.1.0-pre.5` CLI, separate from this case study's
script and documentation commits. The run used macOS 27.0 / Apple M4 / arm64,
one process at a time. The Release archive and `.sha256` sidecar agreed; the
extracted `aarch64-apple-darwin` binary reported `qzt 0.1.0-pre.5` and SHA-256
`989e6f21d80c5f14ee90b27aabe156760f0c6992bc6a5520aaa1400ba20b29be`.

We downloaded the raw job-log bodies with `gh api .../actions/jobs/{job}/logs`
on 2026-09-27 UTC. [`source.json`](source.json) records the acquisition time,
method, run/attempt/job, commit, size, SHA-256, and job/step outcomes. The
tracked files preserve the response bytes (`-text` in `.gitattributes`):

| Job | Commit | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| [pre.4 failed Linux job](https://github.com/albert-einshutoin/qzt/actions/runs/36257908117/job/108448130555) | `4715196614c54c54b7809f99a422343ef2cf7b85` | 34,063 | `1d42a7aae2ba967bf6a39e40dfaaedfd8262ba9e42fa7fd2b3511f0a1004523b` |
| [pre.5 successful Linux job](https://github.com/albert-einshutoin/qzt/actions/runs/36317632199/job/108615277305) | `3bc7d2561c58b59cd166fed75bfbd45f2f4c0ebe` | 38,038 | `70bcd3e7abdf6340ff17f9259f058a68b27bd78c2563b8e8a61475489628de42` |

We fixed the three exact queries in [`queries.json`](queries.json) using a
separate byte scan of the original files **before packing**. Offsets below are
zero-based byte offsets; ranges are half-open. The second document starts at
logical offset 34,063, computed from the first original's length.

## What happened?

1. **Where did pre.4 fail?** The pre.4 job metadata marks **Build artifacts**
   successful and **Verify the dist build kept its selected toolchain** failed.
   Its actual `dist ran successfully` output occurs at `ci/pre4-linux.log`
   byte `29906:29927`; the earlier match at `25026:25047` is the echoed shell
   command, not a success result. The error below is in the later verification
   step. The [pre.4 job](https://github.com/albert-einshutoin/qzt/actions/runs/36257908117/job/108448130555)
   is therefore a failed environment check after a completed build.
2. **What is the failure evidence?** The verified n-gram hit is
   `ci/pre4-linux.log` byte `32023:32060` (logical `32023:32060`). The
   independently selected context `31220:32135` includes `Traceback (most
   recent call last):`, frames in `scripts/record-build-environment.py`
   (`main`, then `snapshot`), `raise RuntimeError("build checkout is dirty")`,
   `RuntimeError: build checkout is dirty`, and exit code 1. The
   [downloaded job](https://github.com/albert-einshutoin/qzt/actions/runs/36257908117/job/108448130555)
   and [`query-0.json`](raw/query-0.json) identify the hit; the raw context is
   retained in [`queries.json`](queries.json).
3. **What changed in the corresponding pre.5 job?** Its **Build artifacts**
   and **Verify the dist build kept its selected toolchain** steps both succeeded.
   The actual `dist ran successfully` output is at `ci/pre5-linux.log`
   `29938:29959`; the echoed command is at `25027:25048`. The environment
   check reports `build environment unchanged:
   3bc7d2561c58b59cd166fed75bfbd45f2f4c0ebe
   x86_64-unknown-linux-gnu` at document byte `31281:31309` (logical
   `65344:65372`). Its context is document `31129:31586` (logical
   `65192:65649`). See the [pre.5 job](https://github.com/albert-einshutoin/qzt/actions/runs/36317632199/job/108615277305)
   and [`query-1.json`](raw/query-1.json). This shows the corresponding check
   succeeded for the pre.5 source; the log alone does not prove every internal
   cause of the pre.4 dirty checkout.

## Preservation and handoff

`pack-docs` made one 15,569-byte QZT with doc IDs `ci/pre4-linux.log` and
`ci/pre5-linux.log`; Deep verify and canonical attestation passed. Verified
`doc` restoration matched each original hash and byte sequence. Full export
matched the ordered 72,101-byte concatenation. One n=3 QZI produced the three
uncapped query results, all exact oracle positions and original byte lengths;
each selected context from `qzt range` equaled the original bytes. The
`dist ran successfully` query has four matches across the two files. A separate
`--max-results 1` run returned one hit with `capped=true` and
`stop_reason=max_search_results`; we did not treat it as exhaustive.
`index_complete_declared=true` and `index_coverage_verified=false` for these
queries. Agreement with this independent scan validates **these files and
queries**, not QZI coverage in general.

A clean second directory received the QZT, attestation, source/query manifests,
script, and published binary, with **no original logs or QZI**. There, Deep
verify passed, a regenerated attestation matched the saved bytes, both `doc`
hashes matched the source manifest, and a rebuilt QZI found the same hits and
contexts. This proves byte integrity and repeatable inspection against the
saved, unsigned manifest. It does not authenticate the log's truth, creator, or
trusted time. See [`raw/recipient-measurements.json`](raw/recipient-measurements.json).

## Cost on this tiny input

| Form | Component bytes | Total bytes |
| --- | --- | ---: |
| Plain | two logs 72,101 + shared source manifest 6,748 | 78,849 |
| tar.gz | archive 15,186 + same manifest 6,748 | 21,934 |
| QZT | container 15,569 + attestation 695 + same manifest 6,748 | 23,012 |
| QZT + QZI | preceding 23,012 + derived n-gram QZI 124,088 | 147,100 |

The optional query manifest is another 5,696 bytes in every investigative
handoff; the published binary and script are separate tooling costs. Keeping
the original logs in Git **as well as** the QZT adds 72,101 bytes before Git
compression. `tar.gz` extraction matched both originals byte-for-byte.

A single sequential run without cache control recorded QZT packing 14.272
ms, Deep verify 2.833 ms, QZI build 18.577 ms, uncapped CLI searches
3.377–3.625 ms each, context ranges 2.402–3.229 ms each, and verified document
restores 2.582/2.628 ms. The tar.gz creation took 3.477 ms. All listed commands
exited 0; [`raw/measurements.json`](raw/measurements.json) has individual
commands, settings, status and wall time. These are process wall times including
CLI launch/output, from one macOS run; no cold-cache, p95/p99, or large-data
claim follows. We did not time an equivalent plain/tar search-to-context
workflow, so the times are not a cross-format search comparison.

The QZT preserved named, verified documents and let us hand over a rebuildable
search path without the source files. For only 72 KB of logs, tar.gz was
slightly smaller than QZT plus attestation, and QZI was much larger than the
input. We still had to acquire job/step metadata separately, calculate document
starts from source lengths, distinguish shell echo from output, and convert
global search offsets to document and context ranges manually in the script.
The CLI's `index_complete_declared` is easy to overread without the separate
`index_coverage_verified` and cap fields. No product function failed in this
case; these workflow costs are candidates for later usability work, not a
reason to change the published product in this task.
