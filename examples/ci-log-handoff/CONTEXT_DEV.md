# Development-only context replay of the pre.5 evidence

This follows the [published pre.5 case study](CASE_STUDY.md); it is not a
release measurement. The QZT and original job-log bytes are unchanged #325
artifacts. The `qzt context` binary was built from development commit
`27203c5493ee8f2e036655ab39d9a07b8c9b0c42` (Issue #327).
Published `v0.1.0-pre.5` and product source
`3bc7d2561c58b59cd166fed75bfbd45f2f4c0ebe` lack this command.

The replay copied **only `evidence.qzt`** into a temporary recipient
directory. `search --index ngram --ngram 3 --format json` returned uncapped
hits from that QZT; each `logical_offset` and `byte_length` went directly to
`context`. The new command used no original logs, `source.json`,
`queries.json`, document-start arithmetic or QZI:

```sh
qzt search evidence.qzt 'build environment unchanged:' --index ngram --ngram 3 --format json
qzt context evidence.qzt --offset 65344 --length 28 --before 2 --after 2 --format json
```

The second command returned `mapping_status=unique`,
`document.id=ci/pre5-linux.log`, local hit `31281:31309`, and original-byte
excerpt `65192:65649` (457 bytes). Both sides stopped as `complete`.
`bytes_hex` is reversible; escaped text is display-only.

| Query / occurrence | Global hit | Document-local hit | Excerpt global span |
|---|---:|---|---:|
| dirty-checkout error | 32023:32060 | pre4 32023:32060 | 30916:32182 |
| unchanged environment | 65344:65372 | pre5 31281:31309 | 65192:65649 |
| dist, pre4 echoed command | 25026:25047 | pre4 25026:25047 | 24703:25144 |
| dist, pre4 actual output | 29906:29927 | pre4 29906:29927 | 29555:30162 |
| dist, pre5 echoed command | 59090:59111 | pre5 25027:25048 | 58759:59239 |
| dist, pre5 actual output | 64001:64022 | pre5 29938:29959 | 63650:64257 |
| fresh `Traceback (most recent call last):` query | 31249:31283 | pre4 31249:31283 | 31097:31447 |

Here `pre4` and `pre5` abbreviate `ci/pre4-linux.log` and
`ci/pre5-linux.log` respectively.
Every row had `before.stop=complete` and `after.stop=complete`. The dirty
error used `--before 12 --after 2` and included the Traceback, `snapshot`
frame, raised error and exit-code line. Others used 2 lines each side.
The four `dist ran successfully` hits still include two echoed commands and
two actual output lines; the command does not decide which is a success record.

Run `cargo test --locked --features internal-testing --test
phase56_context_cli --test phase56_context_case_study`. The case-study test
independently scans tracked original job-log bytes for expected hits and
compares each returned excerpt to its exact source slice. Those logs are
test-oracle inputs only: the recipient directory and `context` process
receive the QZT alone.

This is partial-read evidence. Decoded chunks and the stored Document Index
block are verified; whole-document checksums, search-index coverage, job
metadata and external provenance are not established. Earlier pre.5 raw
records and measurements remain historical results of the published binary.
