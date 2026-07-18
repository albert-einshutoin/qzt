# Fix pipeline artifacts as verified documents

**Time:** 15 minutes  
**Prerequisites:** `qzt 0.1.0-pre.2`; `jq`; `sha256sum` (or macOS
`shasum -a 256`); optional GitHub Actions and `minisign`. The stable command
contract is [docs/CLI.md](../CLI.md).

Use a Document Index when several immutable text outputs must travel together
but still be listed and restored independently.

## 1. Create and pack three artifacts

```sh
set -euo pipefail
umask 077
archive=run-1234.qzt
archive_partial="${archive}.partial"
attestation=run-1234.attest.json
attestation_partial="${attestation}.partial"
for path in report.txt metrics.csv run.log "$archive" "$archive_partial" \
  "$attestation" "$attestation_partial"; do
  test ! -e "$path"
done
trap 'rm -f -- "$archive" "$archive_partial" "$attestation" "$attestation_partial"' EXIT

printf 'Pipeline run 1234\nResult: PASS\nDataset checksum recorded.\n' > report.txt
printf 'metric,value\nrows,1200\nerrors,0\n' > metrics.csv
printf '2026-07-19T02:00:00Z pipeline start\n2026-07-19T02:03:00Z pipeline complete status=PASS\n' > run.log

qzt pack-docs report.txt metrics.csv run.log -o "$archive_partial"
mv -- "$archive_partial" "$archive"
qzt attest "$archive" > "$attestation_partial"
jq -e '.verify.level == "deep"' "$attestation_partial"
mv -- "$attestation_partial" "$attestation"
trap - EXIT
```

`pack-docs` concatenates inputs in argument order without separators. The
default document IDs are unique UTF-8 basenames; use `--doc-id-prefix` when a
namespace is needed. It reads all inputs before packing, so memory grows with
their total size.

## 2. Put the fixation step in GitHub Actions

This complete job is also available as
[`examples/qzt-artifact-workflow.yml`](examples/qzt-artifact-workflow.yml).
Replace the sample input-generation step with the outputs of your pipeline.
The action SHAs are pinned; review and update them with your dependency policy.

```yaml
name: preserve-pipeline-artifacts

on:
  workflow_dispatch:

permissions:
  contents: read

jobs:
  preserve:
    runs-on: ubuntu-latest
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10
        with:
          ref: ${{ github.sha }}
          persist-credentials: false
      - name: Build QZT from the workflow commit
        run: cargo install --locked --path .
      - name: Produce pipeline outputs
        run: |
          printf 'Pipeline report\n' > report.txt
          printf 'metric,value\nrows,1200\n' > metrics.csv
          printf 'pipeline complete\n' > run.log
      - name: Fix, attest, and enforce deep coverage
        env:
          SOURCE_COMMIT: ${{ github.sha }}
          SOURCE_REPOSITORY: ${{ github.repository }}
          WORKFLOW_RUN_ID: ${{ github.run_id }}
          WORKFLOW_RUN_ATTEMPT: ${{ github.run_attempt }}
          WORKFLOW_REF: ${{ github.workflow_ref }}
        run: |
          set -euo pipefail
          umask 077
          test "$(git rev-parse HEAD)" = "$SOURCE_COMMIT"
          for path in run-1234.qzt run-1234.qzt.partial \
            run-1234.attest.json run-1234.attest.json.partial \
            run-1234.provenance.txt run-1234.provenance.txt.partial \
            run-1234.sha256 run-1234.sha256.partial; do
            test ! -e "$path"
          done
          trap 'rm -f -- run-1234.qzt run-1234.qzt.partial run-1234.attest.json run-1234.attest.json.partial run-1234.provenance.txt run-1234.provenance.txt.partial run-1234.sha256 run-1234.sha256.partial' EXIT
          qzt pack-docs report.txt metrics.csv run.log -o run-1234.qzt.partial
          mv -- run-1234.qzt.partial run-1234.qzt
          qzt attest run-1234.qzt > run-1234.attest.json.partial
          jq -e '.verify.level == "deep" and
            .verify.decoded_bytes == .original_size and
            .verify.checked_chunks == .chunk_count' run-1234.attest.json.partial
          mv -- run-1234.attest.json.partial run-1234.attest.json
          {
            printf 'repository=%s\n' "$SOURCE_REPOSITORY"
            printf 'source_commit=%s\n' "$SOURCE_COMMIT"
            printf 'workflow_run_id=%s\n' "$WORKFLOW_RUN_ID"
            printf 'workflow_run_attempt=%s\n' "$WORKFLOW_RUN_ATTEMPT"
            printf 'workflow_ref=%s\n' "$WORKFLOW_REF"
            qzt --version
          } > run-1234.provenance.txt.partial
          mv -- run-1234.provenance.txt.partial run-1234.provenance.txt
          sha256sum run-1234.qzt run-1234.attest.json \
            run-1234.provenance.txt > run-1234.sha256.partial
          mv -- run-1234.sha256.partial run-1234.sha256
          trap - EXIT
      - uses: actions/upload-artifact@b7c566a772e6b6bfb58ed0dc250532a479d7789f
        with:
          name: run-1234-evidence-${{ github.run_id }}
          retention-days: 30
          path: |
            run-1234.qzt
            run-1234.attest.json
            run-1234.provenance.txt
            run-1234.sha256
          if-no-files-found: error
```

The provenance text binds repository, workflow commit, run ID, and QZT version
into the uploaded checksum manifest. An Actions artifact is still only a
transport/retention mechanism, not an external signature. For independent
authenticity or trusted time, authenticate that manifest and sign or timestamp
it as described in the [attestation guide](attestation.md).

## 3. List and restore one verified document

```sh
qzt docs run-1234.qzt --format json | jq '.documents[] | {
  doc_id, logical_offset, byte_length, checksum
}'
qzt doc run-1234.qzt report.txt -o restored-report.txt
cmp report.txt restored-report.txt
```

Validated output included:

```json
{"doc_id":"report.txt","logical_offset":0,"byte_length":58,"checksum":{"algorithm":"blake3","value":"..."}}
```

`qzt doc` verifies the document checksum by default and fails closed. Avoid
`--no-verify` except for explicit diagnosis.

## 4. Audit the container later

Retrieve the preserved files, authenticate the checksum manifest first, verify
its bytes and expected pipeline identity, then reproduce the canonical bytes.
Replace all three expected values with the values authorized for the audit:

```sh
set -euo pipefail
EXPECTED_REPOSITORY=owner/repository
EXPECTED_SOURCE_COMMIT=0123456789abcdef0123456789abcdef01234567
EXPECTED_RUN_ID=1234567890
EXPECTED_RUN_ATTEMPT=1
EXPECTED_WORKFLOW_REF=owner/repository/.github/workflows/preserve.yml@refs/heads/main

minisign -Vm run-1234.sha256 -p minisign.pub
sha256sum -c run-1234.sha256
grep -Fqx "repository=$EXPECTED_REPOSITORY" run-1234.provenance.txt
grep -Fqx "source_commit=$EXPECTED_SOURCE_COMMIT" run-1234.provenance.txt
grep -Fqx "workflow_run_id=$EXPECTED_RUN_ID" run-1234.provenance.txt
grep -Fqx "workflow_run_attempt=$EXPECTED_RUN_ATTEMPT" run-1234.provenance.txt
grep -Fqx "workflow_ref=$EXPECTED_WORKFLOW_REF" run-1234.provenance.txt

qzt verify run-1234.qzt --deep --format json
regenerated=regenerated.attest.json
test ! -e "$regenerated"
trap 'rm -f -- "$regenerated"' EXIT
qzt attest run-1234.qzt > "$regenerated"
diff -u run-1234.attest.json "$regenerated"
test ! -e audited-report.txt
qzt doc run-1234.qzt report.txt -o audited-report.txt
trap - EXIT
```

An empty diff proves that the current, deep-verified container reproduces the
same deterministic attestation bytes. The example assumes the checksum manifest
was signed at fixation time and that `minisign.pub` arrived through a trusted
channel. Without that protected baseline, the local files remain
self-consistent but do not authenticate their creator or detect replacement of
the entire set.

## Limitations

- Inputs must be UTF-8 text, basenames must be unique, and `pack-docs` is not a
  streaming/constant-memory pack path.
- Document checksums prove byte identity within the container, not the truth or
  origin of the content.
- A QZT attestation excludes source paths, host names, and collection time.
  Preserve pipeline identity and external signatures separately.
- The Document Index is immutable; create a new container for updated outputs.
- QZT does not encrypt artifacts. Keep secrets out of inputs where possible and
  restrict who can read workflow logs/artifacts, set retention/deletion policy,
  and apply repository access-control and encryption policy.

The commands and YAML parse were validated. See the
[tutorial validation record](tutorial-validation.md).
