# Extracted-binary public workflow smoke

`scripts/verify-public-workflow.py` selects the fixed **public-workflow-v1**
profile. It requires context; an unsupported command fails rather than being
skipped. Python 3.12+ is required. The runner does not build, download, install
or publish QZT. It executes only the absolute binary path supplied, with
disposable input/output and the existing isolated binary environment.

Published `v0.1.0-pre.5` has no context command. Keep using its existing
`verify-published-release.py` and `smoke-pre5-release-tour.sh` for its published
workflow. This new profile is for a context-enabled main/candidate binary or
a future release that explicitly adopts it. A main build reporting pre.5 is
not the published pre.5 artifact: bind evidence to binary SHA-256 as well as
version. Never infer support from version alone.

First verify the downloaded archive against its release checksum and extract
it. Record the archive origin/checksum and producer source SHA separately;
this smoke checks binary behavior and identity, not publication provenance.
On the matching native host, run (replace every placeholder):

```sh
python3 scripts/verify-public-workflow.py \
  --binary /absolute/path/to/extracted/qzt \
  --binary-sha256 EXPECTED_EXTRACTED_BINARY_SHA256 \
  --expected-tag v0.1.0-pre.5 \
  --target aarch64-apple-darwin \
  --vectors-dir tests/vectors \
  --output /absolute/path/to/fresh-workflow-evidence.json
```

For Windows, use `python`, an absolute `qzt.exe` path, and target
`x86_64-pc-windows-msvc`. Other supported targets are `x86_64-apple-darwin`
and `x86_64-unknown-linux-gnu`. The target must match the actual native host;
cross-compilation does not count as runtime validation. The output path must
be fresh, so an old successful report cannot be mistaken for this run.

The reused candidate smoke checks pack/info/range/QZI/search/deep verify/
attest/export, exact original-byte restoration, result caps, hard errors,
safe file replacement and the LF/CRLF v0.1 golden vectors. Added checks follow
real search-hit coordinates into context, compare the reversible bytes with
the source, distinguish document scope and budget fragments, retain unknown
coverage for ordinary/capped zero hits, and reject a corrupt QZI. It checks
that context does not claim document checksum, query or provenance validation.

Only a fully successful run emits `ok:true` with `profile:public-workflow-v1`.
The report records binary SHA-256/version, native target, the runner and reused
verifier hashes, and hashes of both golden vector files. Check process exit
status before accepting a report. These small fixtures are workflow and
format regression evidence, not a resource envelope, benchmark, comprehensive
corruption test, or evidence of independent real-data adoption.

For the existing pre.3 ↔ pre.5-versioned main/candidate QZT/QZI comparison,
use the unchanged compatibility runner with checksum-verified extracted
binaries:

```sh
python3 scripts/verify-pre4-compatibility.py \
  --pre3-bin /absolute/path/to/published-pre3/qzt \
  --candidate-bin /absolute/path/to/context-enabled-main/qzt \
  --candidate-tag v0.1.0-pre.5 \
  --output /absolute/path/to/compatibility.json
```

It checks both writers with both readers for deep verify/export/attestation,
token/ngram QZI inspection/search and same-QZT QZI byte equality. It does not
test context on pre.3 and is intentionally version-bound. A future release
must update its selected version contract explicitly; do not silently reuse
these version labels. Running this smoke on one host does not prove all four
platforms or the eventual release archive/installer. Run it for each exact
artifact on each native host during the release validation stage.
