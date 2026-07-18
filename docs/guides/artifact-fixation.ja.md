# pipeline成果物を検証可能なdocumentとして固定する

**所要時間:** 15 minutes（約15分）  
**前提:** `qzt 0.1.0-pre.2`、`jq`、`sha256sum`（macOSは`shasum -a 256`）。
GitHub Actionsと`minisign`は任意です。
安定した契約は[docs/CLI.ja.md](../CLI.ja.md)を参照してください。

複数のimmutableなtext出力を一緒に保存しながら、後から個別に一覧・復元したい場合に
Document Indexを使います。

## 1. 3つの成果物を作りpackする

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

`pack-docs`は引数順にseparatorなしで連結します。既定doc IDは一意なUTF-8 basenameです。
namespaceには`--doc-id-prefix`を使います。全inputをpack前に読むため、合計sizeに比例した
memoryを使います。

## 2. GitHub Actionsへ組み込む

完全なjobは[`examples/qzt-artifact-workflow.yml`](examples/qzt-artifact-workflow.yml)
にもあります。sample生成stepをpipeline出力へ置換してください。Action SHAはpin済みです。

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

provenance textでrepository、workflow commit、run ID、QZT versionをuploadするchecksum
manifestへ結び付けます。それでもActions artifactはtransport/retentionであり外部署名では
ありません。独立したauthenticity/trusted timeにはmanifestを認証し、
[attestation guide](attestation.md)に従って署名・timestampします。

## 3. 一覧し、1 documentを検証付き復元する

```sh
qzt docs run-1234.qzt --format json | jq '.documents[] | {
  doc_id, logical_offset, byte_length, checksum
}'
qzt doc run-1234.qzt report.txt -o restored-report.txt
cmp report.txt restored-report.txt
```

検証出力には次が含まれました。

```json
{"doc_id":"report.txt","logical_offset":0,"byte_length":58,"checksum":{"algorithm":"blake3","value":"..."}}
```

`qzt doc`は既定でdocument checksumを検証してfail closedします。`--no-verify`は明示的な
診断時以外に使わないでください。

## 4. 後日監査する

保存fileを取得し、checksum manifestを先に認証してbyteと期待pipeline identityを検証し、
正準byteを再生成します。3つの期待値は監査で承認した値へ置換してください。

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

空のdiffは、現在のdeep-verified containerが同じ決定的attestation byteを再生成したことを
示します。この例はfixation時にchecksum manifestを署名し、`minisign.pub`をtrusted channelで
取得した前提です。保護されたbaselineがなければlocal file同士の整合性しか示せず、set全体の
差し替えや作成者identityは認証できません。

## Limitations（制約）

- inputはUTF-8 text、basenameは一意である必要があり、`pack-docs`はconstant-memoryではありません。
- document checksumはcontainer内のbyte identityを示し、内容の真実性や由来は示しません。
- attestationにsource path、host名、収集時刻は含まれません。pipeline identityは別保管します。
- Document Indexはimmutableです。更新成果物には新しいcontainerを作ります。
- QZTはartifactを暗号化しません。可能ならsecretをinputから除外し、repository、retention、
  workflow log/artifactのviewerを制限し、retention/deletion、repository access control、
  encryption policyを適用します。

コマンドとYAML parseは検証済みです。
[tutorial validation record](tutorial-validation.md)を参照してください。
