# qzt CLI リファレンス (v0.1)

QZT v0.1 technical preview CLIの全コマンドと、自動化向け安定性契約です。
掲載例は[例の再現方法](#例の再現方法)のfixtureで実行確認しています。

English: [CLI.md](CLI.md)

## 安定性契約

### 終了コード

v0.1では次の意味を固定します。

| code | 意味 |
|---:|---|
| `0` | 成功。`verify`では指定レベルの検証成功。 |
| `1` | 実行失敗。読めない/破損した入力、検証失敗、文書なし、I/O失敗など。 |
| `2` | 使用法エラー。未知option、引数不足、不正なoption値。 |

### 機械可読出力

- 明示的な`--format json`が自動化用interfaceです。
- JSON key追加は後方互換です。削除、改名、JSON型または文書化された意味の
  変更はbreaking changeです。consumerは未知keyを無視してください。
- object key順と空白は安定ではありません。ただし`attest`の正準byte列は例外です。
- integerは正確です。浮動小数の表記/精度、`query_time_ms`などの時間値は非安定です。
- textは人間向けです。既存先頭行は可能な限り維持しますが、行を追加できます。
  JSONがある出力をtext parsingしないでください。

### stdoutとstderr

- 成功データはstdout、または`-o`で選んだfileへ出します。
- 使用法エラーと通常の実行エラーはstderrへ出します。
- warningと不完全検索通知はJSON modeでもstderrへ出し、stdoutのJSONを汚しません。
- `verify --format json`の検証失敗だけは例外で、`{"ok":false,...}`を
  stdoutへ1 object出し、stderrは空、終了codeは`1`です。
- `attest`は検証成功まで何も書きません。検証失敗時stdoutは空です。stdoutの
  I/O失敗はstderrへ報告して終了`1`ですが、streamが受理済みのbyteは取り消せず、
  部分出力が残る可能性があります。
- 現在progress出力はありません。将来追加する場合もstderrだけを使用します。

## コマンド

### `qzt help`, `qzt --help`, `qzt --version`

`help`, `-h`, `--help`はtop-level helpを表示して終了`0`です。`-V`と
`--version`は`qzt <version>`を表示して終了`0`です。

### `qzt pack <INPUT|-> -o <OUTPUT> [OPTIONS]`

1つのUTF-8 byte streamを固定します。`INPUT`は必ず最初のcommand引数です。

| option | 意味と既定値 |
|---|---|
| `-o, --output <PATH>` | 必須のQZT出力path。 |
| `--profile <PROFILE>` | `minimal|core|log|archive|memory`。既定`core`。 |
| `--chunk-size <BYTES>` | 目標chunk size。既定4 MiB。 |
| `--max-chunk-size <BYTES>` | 最大chunk size。既定16 MiB。 |
| `--zstd-level <LEVEL>` | zstd level。既定`0`（library default）。 |
| `--checksum blake3` | 受理する唯一のchecksum値。 |
| `--dict none` | 唯一のdictionary mode。CLI dictionary書込は未実装。 |
| `--dense-line-index on\|off` | 既定off。memoryはProfiles節参照。 |
| `-h, --help` | command help。 |

`-`によるstdinはprofile `core`、Dense off、file出力必須のstreaming pathだけで
使えます。seekでoffsetをpatchするためQZTのstdout出力はできません。このpathは
file入力も64 KiBずつ読み、同じdirectoryの一意な一時fileからatomic renameで確定します。
peak memoryはchunk bufferに加えて`O(chunk_count)`のchunk metadataを含み、小さいchunk
設定ほどmetadataが増えます。それ以外は入力全体をmemoryへ読みます。
`qzt pack --profile memory`は必要なDocument Indexを作れないため
終了`1`になります。`pack-docs`を使ってください。

```sh
journalctl --since today | qzt pack - -o today.qzt
```

### `qzt pack-docs <INPUT>... -o <OUTPUT> [OPTIONS]`

指定順でfileを連結し、検証可能なDocument Indexを作ります。stdin非対応です。
文書IDは`<prefix><basename>`で、重複不可です。`pack`と同じoptionに加えて
`--doc-id-prefix <PREFIX>`があります。全入力を先に読むため、総入力sizeに比例する
memoryを使います。memory profileの暗黙既定は目標256 KiB/最大2 MiBです。
明示値が優先され、Dense自動生成は2048行以上（`on|off`で強制可能）です。

```sh
qzt pack-docs alpha.txt beta.txt --doc-id-prefix demo/ -o evidence.qzt
```

### `qzt info <FILE> [--format text|json]`

構造metadataを表示します。既定text。JSON fieldは次のとおりです。

| field | 型 | 意味 |
|---|---|---|
| `format` | string | `qzt-0.1`。 |
| `container_id` | string | 16-byte IDのlowercase hex 32文字。 |
| `profile` | string | 保存されたprofile宣言。 |
| `original_size`, `compressed_size` | integer | 原文と最終containerのbyte数。 |
| `original_checksum` | object | `algorithm`とlowercase hex `value`。 |
| `newline_mode` | string | `none|lf|crlf|mixed`。 |
| `chunk_count`, `line_count` | integer | 保存された件数。 |
| `zstd_level` | integer | writer設定。 |
| `target_chunk_size`, `max_chunk_size` | integer | byte単位writer設定。 |
| `dense_line_index`, `document_index` | boolean | optional block宣言。 |
| `document_count` | integer | Document Indexなしなら0。 |

### `qzt export <FILE> [-o <OUTPUT>]`

全原文byteをstdout、または新規作成/切詰めする出力fileへstreamします。open時にcontainer
構造を検査し、復号時に各chunkの圧縮済み/復号済みchecksumを検証します。container全体の
prefix checksumと原文全体checksumは検証しないため、証跡export前には
`qzt verify <FILE> --deep`を実行してください。

### `qzt range <FILE> --bytes A:B|--lines A:B`

- `--bytes A:B`は0-based半開区間`[A, B)`。
- `--lines A:B`は1-based両端を含む区間`[A, B]`。
- `A <= B`、lineの`A`は1以上。選択した原文byteをstdoutへ出します。

```text
$ qzt range evidence.qzt --bytes 0:15
alpha evidence
$ qzt range evidence.qzt --lines 2:3
shared token
beta evidence
```

### `qzt line <FILE> <LINE> [--zero-based]`

保存された改行を含む1行を読みます。既定1-based、`--zero-based`で0-basedです。

### `qzt docs <FILE> [--format text|json]`

Document Index entryを一覧します。Indexなしは終了`1`。JSONは
`{"documents":[...]}`で、各文書に`doc_id`, `logical_offset`, `byte_length`,
1-based `first_line`, `line_count`, checksumの`algorithm`とlowercase hex
`value`があります。

### `qzt doc <FILE> <DOC_ID> [-o <OUTPUT>] [--no-verify]`

1文書を抽出します。既定ではentry checksumを検証してfail closedします。
`--no-verify`はその文書checksumだけを省略する診断用optionです。`-o`なしはstdout。

### `qzt search <FILE> <QUERY> [OPTIONS]`

検証済み原文UTF-8を検索します。

| option | 意味と既定値 |
|---|---|
| `--index token\|ngram` | memory上raw index。既定`token`。 |
| `--ngram <N>` | n-gram scalar幅。既定3、正数。 |
| `--sidecar <PATH>` | memory構築せず既存QZIを使う。 |
| `--max-candidates <N>` | candidate granule。既定10000。 |
| `--max-decoded-bytes <N|NKiB|NMiB|NGiB>` | decode予算。既定256 MiB。suffixはcase-sensitive。 |
| `--max-results <N>` | 結果上限。既定無制限(`u64::MAX`)。 |
| `--format text\|json` | 既定text。 |

JSON top-levelは`hits` array、`metrics` object、`capped` boolean、
`incomplete_reason` string/nullです。hitは`logical_offset`, `byte_length`,
`chunk_start`, `chunk_end`, `source` (`verified_original_bytes`)を持ちます。
metricsは`query`, `index_kind`, `posting_granularity`, `index_size_bytes`,
`source_size_bytes`, `index_size_ratio`, `term_lookups`, `posting_bytes_read`,
`candidate_granules`, `candidate_chunks`, `decoded_bytes`,
`physical_decoded_bytes`, `verified_matches`, `query_time_ms`です。

`incomplete_reason`は現在`query_shorter_than_ngram_n`,
`query_has_no_indexable_tokens`, `missing_required_key_in_incomplete_index`です。
null以外なら、空/部分結果を完全な否定結果として解釈してはいけません。

### `qzt sidecar-rebuild <FILE> -o <OUTPUT.qzi> [OPTIONS]`

QZIを作ります。`--index token|ngram`（既定token）、`--ngram <N>`（既定3）、
必須`-o, --output`。searchで開く際に対象containerとの対応を検証します。

### `qzt verify <FILE> [--quick|--normal|--deep] [--format text|json]`

既定normal。level flagが複数なら最後が優先です。

| level | 検証内容 |
|---|---|
| `quick` | 構造block、offset、schema、必須checksum、resource limit。 |
| `normal` | quick＋保存圧縮chunk checksum。decoded bytesは0。 |
| `deep` | normal＋復号、原文checksum、UTF-8/newline/index/document整合。 |

成功JSONは`ok`, `level`, `checked_chunks`, `decoded_bytes`。失敗JSONは
`ok:false`, `level`, `error`を持ち、安定性契約どおりstdoutへ出して終了`1`です。

```json
{"ok":true,"level":"deep","checked_chunks":1,"decoded_bytes":55}
```

### `qzt attest [--level quick|normal|deep] <FILE>`

既定deep。optionはfileの前後どちらでも使えます。検証成功まで何も出さず、成功後に
正準JSON 1行だけを書きます。top-level fieldは`chunk_count`, `container_checksum`,
`container_id`, `final_file_size`, `format`, `line_count`, `original_checksum`,
`original_size`, `verify`で、nested `verify`は`checked_chunks`, `decoded_bytes`, `level`を
持ちます。[アテステーション正準形](#アテステーション正準形)と
[署名guide](guides/attestation.md)を参照してください。

## プロファイル

| profile | v0.1での実挙動 |
|---|---|
| `minimal` | metadataで用途宣言。CLIは全入力path。optional index既定なし。 |
| `core` | 既定。Dense offの単一入力packはpayloadをstreamします。memoryはchunk bufferと`O(chunk_count)` metadataで、定数memory SLAではありません。 |
| `log` | metadata用途宣言以外は同じoptionのcoreと同じ物理layout。全入力path。 |
| `archive` | metadata用途宣言以外は同じoptionのcoreと同じ物理layout。全入力path。 |
| `memory` | Document Index必須のため`pack-docs`を使う。取得向けchunk既定と2048行以上のDense自動生成。 |

v0.1の`minimal`, `log`, `archive`は別compression algorithmではなく正直な用途宣言です。
主な物理差はprofile名だけでなくchunk設定とoptional indexから生じます。

## JSON例

fixtureの`info` identityです（空白は契約外）。

```json
{"format":"qzt-0.1","container_id":"ea4b7a560231e640c9ab0c838cc22a78","profile":"core","original_size":55,"compressed_size":2536,"original_checksum":{"algorithm":"blake3","value":"ea4b7a560231e640c9ab0c838cc22a7813bbc864d5a9f8a850df7ca5960dff30"},"newline_mode":"lf","chunk_count":1,"line_count":4,"zstd_level":0,"target_chunk_size":4194304,"max_chunk_size":16777216,"dense_line_index":false,"document_index":true,"document_count":2}
```

`docs --format json`はoffset 0/length 28/first line 1の`demo/alpha.txt`と、
offset 28/length 27/first line 3の`demo/beta.txt`を返します。
`search shared --format json`はlogical offset 15と42のverified hitを返しました。
非安定な時間値・浮動小数表記はここでは省略します。

## アテステーション正準形

他のJSONと異なり、attest byte列は署名可能な安定契約です。

- 無意味な空白なしの1 object/1 line。
- top-level/nested keyを辞書順。
- lowercase hex、JSON integer、legacyで`container_checksum`がない場合だけ`null`。
- path/host/clock/locale等の環境依存値なし。
- 末尾LFちょうど1つ。
- fieldは`chunk_count`, `container_checksum`, `container_id`,
  `final_file_size`, `format`, `line_count`, `original_checksum`,
  `original_size`, `verify` (`checked_chunks`, `decoded_bytes`, `level`)。

実行済みfixture出力:

```json
{"chunk_count":1,"container_checksum":{"algorithm":"blake3","value":"c0c832eeb45e889673968b846e66abd9a533ccee5c6aa229f521486e195acbd1"},"container_id":"ea4b7a560231e640c9ab0c838cc22a78","final_file_size":2536,"format":"qzt-0.1","line_count":4,"original_checksum":{"algorithm":"blake3","value":"ea4b7a560231e640c9ab0c838cc22a7813bbc864d5a9f8a850df7ca5960dff30"},"original_size":55,"verify":{"checked_chunks":1,"decoded_bytes":55,"level":"deep"}}
```

## 制限

- CLI dictionary書込は未実装で、`--dict none`だけを受理します。
- normalized Unicode searchはありません。raw tokenはASCII英数字、ngramはraw
  UTF-8/scalarです。
- QZT stdout出力はseekが必要なため未対応です。
- `pack-docs`は非streamingで、basename由来UTF-8 IDは一意である必要があります。
- technical previewです。明示したv0.1契約は安定ですが、未規定の表示・性能詳細は非安定です。

## 例の再現方法

repository rootから実行してください。掲載出力は次のLF入力とrepository binaryで
実行しました。

```sh
set -eu
cargo build --all-features --bin qzt
QZT_BIN="$(pwd)/target/debug/qzt"
QZT_EXAMPLE_DIR="$(mktemp -d)"
trap 'rm -rf -- "$QZT_EXAMPLE_DIR"' EXIT
cd "$QZT_EXAMPLE_DIR"
printf 'alpha evidence\nshared token\n' > alpha.txt
printf 'beta evidence\nshared token\n' > beta.txt
"$QZT_BIN" pack-docs alpha.txt beta.txt --doc-id-prefix demo/ -o evidence.qzt
"$QZT_BIN" info evidence.qzt --format json
"$QZT_BIN" verify evidence.qzt --deep --format json
"$QZT_BIN" range evidence.qzt --bytes 0:15
"$QZT_BIN" range evidence.qzt --lines 2:3
"$QZT_BIN" line evidence.qzt 1
"$QZT_BIN" docs evidence.qzt --format json
"$QZT_BIN" doc evidence.qzt demo/beta.txt
"$QZT_BIN" search evidence.qzt shared --format json
"$QZT_BIN" sidecar-rebuild evidence.qzt --index token -o evidence.qzi
"$QZT_BIN" search evidence.qzt shared --sidecar evidence.qzi --format json
"$QZT_BIN" attest evidence.qzt --level deep
"$QZT_BIN" export evidence.qzt -o exported.txt
```

最後にexport fileと2入力の連結をbyte単位で比較しました。`query_time_ms`は再実行で変化し、
安定性契約どおりです。
