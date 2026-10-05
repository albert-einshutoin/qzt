# Public artifact workflow

[日本語](public-workflow.ja.md)

The published context-enabled preview is **v0.1.0-pre.6**, from product source
`0c8110e6b6e08513b4def3f636b5b79d068e275f`. The [actual Release URL verification](../releases/v0.1.0-pre.6-published.md)
passed the C4 profile on all four native targets, including native Windows
PowerShell installation. The literal README/guide PowerShell commands also
passed natively on Windows. No source checkout or Rust build is needed.

## Select and install

| Host | Target | Archive |
|---|---|---|
| macOS Apple silicon | `aarch64-apple-darwin` | `.tar.xz` |
| macOS Intel | `x86_64-apple-darwin` | `.tar.xz` |
| Linux x64, glibc | `x86_64-unknown-linux-gnu` | `.tar.xz` |
| Windows x64 | `x86_64-pc-windows-msvc` | `.zip` |

Use a fresh directory and the [fixed-tag archive instructions](../../README.md#install).
They download both `qzt-<target>` archive and matching `.sha256`, verify before
extraction, set an absolute binary path and require the selected version.
Do not substitute `latest` or an arbitrary `qzt` on PATH. POSIX installation
requires HTTPS `curl`, `tar` with xz support, and `shasum` or `sha256sum`.
Windows installation uses PowerShell 5.1+ with `Invoke-WebRequest`,
`Get-FileHash`, `Expand-Archive` and .NET. Keep tag, source commit, archive
checksum and binary version with your results. The release channel supplies
the checksum's trust; checksum equality alone does not authenticate a publisher.

## Run the basic workflow

For POSIX, run the [README tour](../../README.md#60-second-tour) using `QZT_BIN`
from installation. It requires `mktemp` and `cmp` and operates on a disposable
23-byte input. For PowerShell, after the Windows installation sets `$QztBin`:

```powershell
function Invoke-Qzt {
    & $QztBin @args
    if ($LASTEXITCODE -ne 0) { throw "qzt failed with exit $LASTEXITCODE" }
}
$tourDir = Join-Path ([IO.Path]::GetTempPath()) ("qzt-tour-" + [Guid]::NewGuid())
New-Item -ItemType Directory $tourDir | Out-Null
Set-Location $tourDir
$original = [Text.Encoding]::UTF8.GetBytes("alpha`nbeta`nerror gamma`n")
[IO.File]::WriteAllBytes((Join-Path $tourDir "app.log"), $original)
Invoke-Qzt pack app.log -o app.qzt
Invoke-Qzt info app.qzt --format json
Invoke-Qzt sidecar-rebuild app.qzt -o app.qzi
Invoke-Qzt inspect-sidecar app.qzt --sidecar app.qzi --format json
$search = Invoke-Qzt search app.qzt error --sidecar app.qzi --format json | ConvertFrom-Json
$search | ConvertTo-Json -Depth 8
$verify = Invoke-Qzt verify app.qzt --deep --format json | ConvertFrom-Json
if ($verify.ok -ne $true -or $verify.level -ne 'deep' -or
    $verify.original_checksum_verified -ne $true) { throw "Deep verification did not pass" }
Invoke-Qzt export app.qzt -o restored.log
$restored = [IO.File]::ReadAllBytes((Join-Path $tourDir "restored.log"))
if ([Convert]::ToBase64String($original) -cne [Convert]::ToBase64String($restored)) {
    throw "Restored bytes differ"
}
"Original/restored bytes: $($original.Length); equality: PASS"
```

Expected: info reports `qzt-0.1`, 23 bytes and 3 lines. Search returns
`error` at global byte offset 11, length 5, `source=verified_original_bytes`.
`index_coverage_verified=false` means coverage unknown even with no cap.
Deep verify reports `ok=true` and `original_checksum_verified=true`.
Export matches every original byte. The base64 comparison above is for this
tiny fixture, not a recommended comparison for large inputs.

## Full context workflow

Use the exact pre.6 binary installed above. The real Release assets passed W3. Context is mandatory.

POSIX requires `jq` and Python 3 in addition to the tools above. Start with the
verified context-enabled `QZT_BIN` and run:

```sh
set -eu
"$QZT_BIN" context --help >/dev/null
tour_dir="$(mktemp -d)"
cd "$tour_dir"
printf 'alpha\nbeta\nerror gamma\n' > app.log
"$QZT_BIN" pack app.log -o app.qzt
"$QZT_BIN" info app.qzt --format json
"$QZT_BIN" sidecar-rebuild app.qzt -o app.qzi
"$QZT_BIN" inspect-sidecar app.qzt --sidecar app.qzi --format json
"$QZT_BIN" search app.qzt error --sidecar app.qzi --format json > search.json
jq -e '.capped == false and .stop_reason == null and .incomplete_reason == null and
  .index_coverage_verified == false and (.hits | length) == 1 and
  .hits[0].source == "verified_original_bytes"' search.json >/dev/null
offset="$(jq -er '.hits[0].logical_offset' search.json)"
length="$(jq -er '.hits[0].byte_length' search.json)"
"$QZT_BIN" context app.qzt --offset "$offset" --length "$length" --format json > context.json
cat context.json
python3 -c 'import json,sys; r=json.load(sys.stdin); sys.stdout.buffer.write(bytes.fromhex(r["excerpt"]["bytes_hex"]))' < context.json > context.log
cmp app.log context.log
"$QZT_BIN" verify app.qzt --deep --format json > verify.json
jq -e '.ok == true and .level == "deep" and .original_checksum_verified == true' verify.json >/dev/null
"$QZT_BIN" export app.qzt -o restored.log
cmp app.log restored.log
```

For PowerShell, reuse `Invoke-Qzt`, set `$QztBin` to the verified
context-enabled artifact, then create a fresh tour with the same 23-byte
input and run pack/info/sidecar-rebuild/inspect-sidecar/search as above.
Insert these commands **after search and before deep verify/export**:

```powershell
Invoke-Qzt context --help | Out-Null
if ($search.capped -ne $false -or $null -ne $search.stop_reason -or
    $null -ne $search.incomplete_reason -or $search.index_coverage_verified -ne $false -or
    @($search.hits).Count -ne 1 -or $search.hits[0].source -ne 'verified_original_bytes') {
    throw "Unexpected search state"
}
$hit = $search.hits[0]
$context = Invoke-Qzt context app.qzt --offset $hit.logical_offset --length $hit.byte_length --format json | ConvertFrom-Json
$context | ConvertTo-Json -Depth 8
$hex = $context.excerpt.bytes_hex
[byte[]]$contextBytes = for ($i = 0; $i -lt $hex.Length; $i += 2) {
    [Convert]::ToByte($hex.Substring($i, 2), 16)
}
if ([Convert]::ToBase64String($original) -cne [Convert]::ToBase64String($contextBytes)) {
    throw "Context bytes differ"
}
```

This fixture's default context covers all 23 bytes. `mapping_status` is
`no_document_index`, scope is `container`, before stop is `complete`, after
stop is `scope_boundary`, and fragment flags are false. With document-aware
input use `pack-docs`; inspect `mapping_status` and scope before assigning
ownership. Arbitrary data or capped context need not match the entire input.

## Interpret results and recover from errors

Follow the [result contract](../CLI.md#interpreting-search-context-and-verify-results):
hit correctness, search completeness, context scope and verify level are
independent. A zero-hit result does not prove absence. For context, inspect
both side stops and fragment flags; `bytes_hex` is reversible while
`text_escaped` is display-only. `verify --deep` does not inspect QZI.

Exit `1` is failure and `2` is usage error; stop instead of treating either
as zero hits. `verify --format json` writes its failure object to stdout.
If a sidecar is rejected, preserve the QZT, deep-verify it and explicitly
rebuild a separate QZI with `sidecar-rebuild`; do not silently replace or
trust the damaged sidecar. See [search operations](search-operations.md).
CLI dictionary writing is not supported; do not select it for this tour.
These small examples do not establish large-data memory, disk or time limits.

For engineers, the [C4 extracted-binary smoke](public-workflow-smoke.md)
checks the fixed profile with expected binary hash/version, golden vectors
and fresh JSON evidence. Candidate checks run all four native targets and the
Windows PowerShell installer; their results do not establish public availability.
W3 repeated the same profile on the real Release URLs; see the published record above.
