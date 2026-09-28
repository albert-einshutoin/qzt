# Published pre.5 CI log handoff

This directory fixes two downloaded GitHub Actions **job log response bodies** as
UTF-8 bytes. They retain their BOM, timestamps, ANSI escapes, and LF bytes. The
job and step outcomes are metadata in [`source.json`](source.json), not text
inserted into either log. `queries.json` was made by `oracle.py` scanning those
two downloaded files before QZT packing. See the [English case study](CASE_STUDY.md)
or [Japanese case study](CASE_STUDY.ja.md).

The published CLI is `v0.1.0-pre.5`, product source
`3bc7d2561c58b59cd166fed75bfbd45f2f4c0ebe`. On macOS arm64, download
`qzt-aarch64-apple-darwin.tar.xz` and its `.sha256` from the
[Release](https://github.com/albert-einshutoin/qzt/releases/tag/v0.1.0-pre.5),
check the sidecar, extract `qzt`, and check its version and SHA-256 against
`source.json`. This case used archive SHA-256
`3cbcab43f1c79ff11b756af292524715d1ca618c0445e4157e775876e454a9d1`
and extracted binary SHA-256
`989e6f21d80c5f14ee90b27aabe156760f0c6992bc6a5520aaa1400ba20b29be`.
The script refuses a different binary.
The checked archive, sidecar, and runtime details are in
[`raw/release-check.json`](raw/release-check.json).

From the repository root, with the extracted binary at `$QZT_BIN`:

```sh
python3 -m unittest discover -s examples/ci-log-handoff -p 'test_*.py'
python3 examples/ci-log-handoff/oracle.py
python3 examples/ci-log-handoff/run.py produce --binary "$QZT_BIN" --work /tmp/qzt-ci-log-work
```

`--work` must name a new directory. The producer validates the downloaded
inputs and fixed oracle, creates one QZT and one n=3 QZI, and records every
command, exit code, and wall time under `/tmp/qzt-ci-log-work/raw`. Its
`bundle` contains the QZT, attestation, source and query manifests, and the
derived QZI. The tracked `evidence.qzt` and `evidence.attest.json` are the
producer's saved output. `raw/` holds selected unedited CLI JSON and the
execution record; the full restored logs remain temporary.

To replay in a separate directory, **copy only** the four listed evidence files,
the script, and the published executable. Do not copy original logs or QZI:

```sh
mkdir /tmp/qzt-ci-log-recipient
cp /tmp/qzt-ci-log-work/bundle/evidence.qzt \
  /tmp/qzt-ci-log-work/bundle/evidence.attest.json \
  examples/ci-log-handoff/source.json examples/ci-log-handoff/queries.json \
  examples/ci-log-handoff/run.py "$QZT_BIN" /tmp/qzt-ci-log-recipient/
cd /tmp/qzt-ci-log-recipient
python3 run.py receive --binary ./qzt --bundle .
```

The recipient verifies and restores both documents before rebuilding QZI. Its
query checks use the manifest's expected byte ranges and context bytes. The
manifest and attestation are unsigned: they show local consistency against
the saved baseline, not who created the logs or when.

The unreleased development `qzt context` command has a separate
[context replay](CONTEXT_DEV.md) ([日本語](CONTEXT_DEV.ja.md)). Its commit and
checks are distinct from the published pre.5 run and raw measurements above.
