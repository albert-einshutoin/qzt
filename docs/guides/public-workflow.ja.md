# 公開artifactからのworkflow導入

[English](public-workflow.md)

context対応previewの選択versionは **v0.1.0-pre.6**、現在は**未公開candidate**です。
以下はこのversionのartifactだけを使う手順です。公開後に実行してください。
W2 candidateの証拠と、W3の実Release URLの証拠は別です。
[release gate](../releases/v0.1.0-pre.6-candidate.md)を参照してください。
公開workflowにsource checkoutやRust buildは不要です。

## artifactを選んで導入する

| Host | Target | Archive |
|---|---|---|
| macOS Apple silicon | `aarch64-apple-darwin` | `.tar.xz` |
| macOS Intel | `x86_64-apple-darwin` | `.tar.xz` |
| Linux x64、glibc | `x86_64-unknown-linux-gnu` | `.tar.xz` |
| Windows x64 | `x86_64-pc-windows-msvc` | `.zip` |

新しいdirectoryで[固定tagのarchive導入手順](../../README.ja.md#install--インストール)を
実行します。`qzt-<target>` archiveと対応する`.sha256`を取得し、検証してから展開し、
binaryの絶対パスと期待versionを確認します。`latest`やPATH上の任意の`qzt`で
置き換えないでください。POSIXではHTTPSの`curl`、xz対応`tar`、`shasum`または
`sha256sum`が必要です。WindowsではPowerShell 5.1以降の`Invoke-WebRequest`、
`Get-FileHash`、`Expand-Archive`と.NETを使います。結果にはtag、source commit、
archive checksum、binary versionを記録してください。checksumの信頼はreleaseの
取得経路に依存し、一致だけでpublisherの真正性は証明しません。

## 基本workflowを実行する

POSIXでは導入時の`QZT_BIN`で[READMEツアー](../../README.ja.md#60秒ツアー)を
実行します。`mktemp`と`cmp`を使い、使い捨ての23-byte入力を作ります。
PowerShellではWindows導入で設定した`$QztBin`を使い、以下を実行します。

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

期待結果：infoは`qzt-0.1`、23 bytes、3行です。searchはglobal byte offset 11、
length 5の`error`を返し、`source=verified_original_bytes`です。
`index_coverage_verified=false`はcapがなくても網羅性がunknownであることを示します。
Deep verifyは`ok=true`、`original_checksum_verified=true`を返し、exportは原文の
全byteと一致します。上のbase64比較はこの小さなfixture用で、大容量入力には推奨しません。

## contextを含む全workflow

上で導入した正確なpre.6 binaryを使います。公開前は準備済み手順であり、
W3で実Release artifactを確認する必要があります。contextは必須です。

POSIXでは上記の道具に加えて`jq`とPython 3を使います。検証済みのcontext対応
`QZT_BIN`から開始します。

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

PowerShellでは`Invoke-Qzt`を再利用し、`$QztBin`を検証済みcontext対応artifactへ
設定します。新しいtour directoryと同じ23-byte入力を作り、上の
pack/info/sidecar-rebuild/inspect-sidecar/searchを実行します。
以下を **searchの後、deep verify/exportの前** に挿入します。

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

このfixtureのdefault contextは23 bytes全体です。`mapping_status`は
`no_document_index`、scopeは`container`、beforeのstopは`complete`、afterは
`scope_boundary`、fragment flagは両方falseです。documentを持つ入力には
`pack-docs`を使い、所有documentを判断する前にmappingとscopeを確認します。
任意の入力やcapで切られたcontextでは、入力全体と一致するとは限りません。

## 結果の読み方とエラー後の操作

[結果判定契約](../CLI.ja.md#searchcontextverifyの結果判定)に従ってください。
hitの正しさ、検索網羅性、context scope、verify levelは独立です。0件は原文での
不存在の証明ではありません。contextは両側のstopとfragment flagを確認し、byte復元には
`bytes_hex`を使います。`text_escaped`は表示用です。`verify --deep`はQZIを検査しません。

終了`1`は失敗、`2`はusage errorなので、0件と扱わず停止します。
`verify --format json`は失敗objectをstdoutへ出します。sidecarが拒否された場合は
QZTを保持してdeep verifyし、`sidecar-rebuild`で別のQZIを明示的に作ります。
破損sidecarを暗黙に置き換えたり信用しないでください。[検索運用guide](search-operations.ja.md)
を参照してください。CLIによるdictionary書込みは未対応なので、このtourでは選びません。
小さな例から大容量のRAM、disk、実行時間の上限は証明できません。

Engineering向けの[C4展開済みbinary smoke](public-workflow-smoke.md)は期待hash/version、
golden vector、新しいJSON証拠fileを要求します。macOS ARM64のmain build証拠は
公開release証拠とは別です。このguideのcandidateは4 native targetとWindows PowerShell installerで確認し、
W3で実Release URLから同じprofileを再実行します。candidate結果は公開到達性の証拠ではありません。
