# qzt CLI リファレンス (開発版CLI・QZT形式v0.1)

このページはcommit `ad709214f1e8ae18eff6e9f0b633345e40d1617b`の
開発版CLIと自動化向け契約を説明します。導入方法は
[READMEの開発版手順](../README.ja.md#開発版cli)を参照してください。
公開済み`v0.1.0-pre.2` binaryには一部のcommand・JSON fieldがありません。
[公開タグ固定のCLIリファレンス](https://github.com/albert-einshutoin/qzt/blob/v0.1.0-pre.2/docs/CLI.ja.md)
を参照してください。`v0.1`はCLI配布versionではなくcontainer形式です。
掲載例は[例の再現方法](#例の再現方法)のfixtureを使います。

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

### file出力の保護

`pack`、`pack-docs`、`export`、`doc`、`sidecar-rebuild`は、いずれかの入力と
同じfileを出力先に指定すると書込み前に拒否します。入力側のlinkをたどり、
filesystem上の同一性（Unixはdevice/inode、Windowsはvolume/file ID）を比較するため、
相対/絶対path、hard link、case-insensitive filesystemの大文字小文字違いも対象です。
出力先がsymlinkなら、dangling symlinkも含め、参照先にかかわらず拒否します。

これらのcommandは出力先と同じdirectoryに一意な一時fileを作り、既存出力の
modeとアクセスACLを引き継ぎます。書込み・検証・flush・file syncに成功してから
置換します。Windowsで既存出力にreadonly属性がある場合は、一時file作成前に
明確なエラーで拒否し、内容・readonly属性・DACLを変更しません。書込み可能な
既存出力の置換と新規出力は通常どおり実行します。
置換前の失敗では入力と既存出力は変わらず、新規出力の完成名に部分fileは残りません。
一時fileの清掃にも失敗した場合は、主エラーと残った一時pathを両方stderrに出します。
置換失敗はexit `1`で結果の確認を求めます。**置換後**のdirectory永続化確認が
失敗した場合もexit `1`で「置換済み・永続化未確認」を明示します。この場合、
既存出力が保全されたと解釈してはいけません。

アクセスACLはmacOSでは`fcopyfile`、Linuxでは`system.posix_acl_access`、
Windowsでは既存出力のsecurity descriptorからDACLを複製します。複製できない
場合は置換しません。owner、その他の拡張属性、filesystem固有のsecurity labelは
引き継ぎ対象外です。macOS/Linuxは同一directory内のrename後、親directoryを`sync_all`します。
Windowsは同一directory内で`MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)`を使います。
Windowsに移植可能な独立したdirectory syncはありません。APIの成功は置換完了を示しますが、
`WRITE_THROUGH`の明示的なflush保証はcopy/delete経路に関するもので、今回の同一volume内の
renameには適用を証明できません。Windowsで成功してもdirectory entryの障害時永続性は未確認です。
filesystemやnetwork mountの種類を問わない停電耐性は保証せず、
操作中の他processによるpath変更は対象外です。stdoutはstreamであり、失敗前に
受理されたbyteは取り消せません。

## コマンド

### `qzt help`, `qzt --help`, `qzt version`, `qzt --version`

`help`, `-h`, `--help`はtop-level helpを表示して終了`0`です。`-V`と
`--version`、および`version` commandは`qzt <version>`を表示して終了`0`です。

### `qzt pack <INPUT|-> -o <OUTPUT> [OPTIONS]`

1つのUTF-8 byte streamを固定します。optionは`INPUT`の前後どちらにも指定できます。

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
chunk設定、生成index、document metadataがdefault Readerの上限を超える場合は
packをエラーにします。streaming Writerには空のrandom-access一時fileを渡します。
generic APIは非空sinkを書込み前に拒否し、途中の書込み・flush失敗では部分出力が
残り得ます。CLIはその一時出力を廃棄し、既存の出力先を保全します。generic sinkの
flush成功は停電時の永続化保証ではなく、CLI側で別途file syncと置換を行います。

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

全原文byteをstdout、またはatomicに置換する出力fileへstreamします。open時にcontainer
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
| `--max-query-bytes <N|NKiB|NMiB|NGiB>` | queryのUTF-8 byte数。sidecarなしのindex構築前に確認。既定4 KiB。 |
| `--max-query-terms <N>` | 重複除去後のtoken/ngram key数。既定256。 |
| `--max-posting-bytes <N|NKiB|NMiB|NGiB>` | queryで扱う実際の符号化posting byte数。既定128 MiB。 |
| `--max-posting-ids <N>` | 選択したposting ID総数。既定10,000,000。 |
| `--max-posting-work <N>` | IDコピー・比較・交差結果への追加の総数。既定20,000,000。 |
| `--max-candidates <N>` | candidate granule。既定10000。 |
| `--max-decoded-bytes <N|NKiB|NMiB|NGiB>` | 検証するlogical候補byteと隣接token境界byte。既定256 MiB。 |
| `--max-physical-decoded-bytes <N|NKiB|NMiB|NGiB>` | 物理的に展開する完全chunkのbyte数。既定256 MiB。 |
| `--max-physical-decoded-chunks <N>` | chunk展開回数。既定10,000。 |
| `--max-line-bytes <N|NKiB|NMiB|NGiB>` | sidecarなしのindex構築時の行byte数。既定16 MiB。 |
| `--max-results <N>` | 結果上限。既定10,000。 |
| `--format text\|json` | 既定text。 |

byte suffixは大文字小文字を区別します。0は該当単位の作業を許しません。
query・posting・index構築の超過は成功reportを出さずexit `1`です。
candidate・logical/physical decode・結果上限は検証済みhitだけを含む理由付きcapです。
課金単位と判定位置は[budget表](QZT_v0.1_Memory_Guarantees.md#search-and-index-build-budgets)を参照してください。

QZI 検索では、取得した granule の範囲を紐づく QZT Chunk Table に照合してから
候補 chunk 数や hit 座標に使用します。file-backed 検索が候補上限で granule
取得前に終了した場合、`candidate_chunks` は `0` で、未読 record は未検証です。

JSON top-levelは`hits` array、`metrics` object、`capped` boolean、
`stop_reason` string/null、`index_complete_declared` boolean、
`index_coverage_verified` boolean、`incomplete_reason` string/nullです。hitは`logical_offset`, `byte_length`,
`chunk_start`, `chunk_end`, `source` (`verified_original_bytes`)を持ちます。
metricsは`query`, `index_kind`, `posting_granularity`, `index_size_bytes`,
`source_size_bytes`, `index_size_ratio`, `term_lookups`, `posting_bytes_read`,
`candidate_granules`, `candidate_chunks`, `decoded_bytes`,
`physical_decoded_bytes`, `physical_decoded_chunks`, `verified_matches`, `query_time_ms`です。

textのmetricsにも両index fieldと同じ`stop_reason`を出します（理由がなければ`none`）。
`index_complete_declared`はmemory indexのflagまたはQZI manifestの宣言で、
原文の網羅性の証明ではありません。`index_coverage_verified`は現在falseです。
`complete=true`や通常の0件でも同じです。section checksumとsource bindingでは
全一致箇所にpostingがあるとは証明できません。`source=verified_original_bytes`は
返したhitだけの保証です。token hitでは全query tokenが原文の同じ行にあり、必要なら
granule外側のbyteも読んでtoken境界を検証します。このbyteはlogical/physical
decode予算に計上します。
`capped=true`は理由を持ち、capによる0件と通常の0件を区別できます。
一時的なn-gram indexの`posting_bytes_read`はplannerの推定値で、実処理予算の計数値ではありません。

`incomplete_reason`は現在`query_shorter_than_ngram_n`,
`query_has_no_indexable_tokens`, `missing_required_key_in_incomplete_index`です。
null以外なら、空/部分結果を完全な否定結果として解釈してはいけません。
理由がnullでcapがなくても、未検証のindexから不存在は証明できません。
結果上限は到達時に停止し、その先にhitがあるかは分かりません。

### `qzt inspect-sidecar <FILE.qzt> --sidecar <FILE.qzi> [--format text|json]`

QZTを`QztFileReader`で開き、全QZI sectionのchecksumとsource bindingを検証してから
metadataを表示します。既定のtext出力とJSON出力は`index_type`、`ngram_n`、
`complete`、`high_df_per_million`、`source_size_bytes`、`index_size_bytes`、
`granule_count`、`term_count`、`postings_size_bytes`を含みます。破損または別QZTに
紐づくsidecarは成功summaryを出さずexit `1`になります。inspectionが行うQZT検証は
quick構造検証までです。Core全体の検証には`qzt verify <FILE.qzt> --deep`を使います。
`complete`はsidecar manifestの宣言値で、検索の網羅性の検証結果ではありません。
inspection成功でも全一致箇所にpostingがあるとは証明できません。

### `qzt sidecar-rebuild <FILE> -o <OUTPUT.qzi> [OPTIONS]`

QZIを作ります。`--index token|ngram`（既定token）、`--ngram <N>`（既定3）、
`--max-line-bytes <N|NKiB|NMiB|NGiB>`（既定16 MiB、LFと直前のCRを含む）、
必須`-o, --output`。行上限超過はkey生成前に拒否します。
searchで開く際に対象containerとの対応を検証します。

### `qzt verify <FILE> [--quick|--normal|--deep] [--format text|json]`

既定normal。level flagが複数なら最後が優先です。

| level | 検証内容 |
|---|---|
| `quick` | 構造block、offset、schema、必須checksum、resource limit。 |
| `normal` | quick＋全圧縮chunk checksumと、存在する場合のcontainer prefix checksum。decoded bytesは0。 |
| `deep` | normal＋復号、原文checksum、UTF-8/newline/index/document整合。 |

成功textは従来の先頭3行を維持し、圧縮checksum照合chunk数、展開chunk数、原文checksum状態、
optionalなprefix checksum／Dense Line Index／Document Indexの状態を追加します。
成功JSONは既存の`ok`, `level`, `checked_chunks`, `decoded_bytes`に加え、
`compressed_checksum_chunks`, `decoded_chunks`, `original_checksum_verified`,
`container_checksum_status`, `dense_line_index_status`, `document_index_status`を返します。
`checked_chunks`は全levelでopen時に構造確認したChunk Table entry数です。payload検証数では
ありません。chunk counterは内部で再hashした回数ではなく、異なるchunkの数です。
同じReaderで以前にDeepを実行していても、今回指定したlevelの結果だけを表します。

prefixの状態は`absent`／`present_unchecked`／`verified`、indexの状態は
`absent`／`stored_block_verified`／`source_checked`です。保存blockの状態はblock checksum、
schema、containerとのbinding、descriptorの物理保存範囲の検証を意味します。
この段階ではDocument Indexの論理範囲やchunk spanは未照合です。
DeepではDLIのoffsetを復号byteと照合します。
Document Indexは文書byteのhash、論理範囲境界、chunk spanを照合しますが、正確な行位置・
行数まで原文と照合したという意味ではありません。未知のoptional blockは対象外です。
prefix checksumの対象はFooter Payload直前までで、全ファイルではありません。

失敗JSONは
`ok:false`, `level`, `error`を持ち、安定性契約どおりstdoutへ出して終了`1`です。

```json
{"ok":true,"level":"deep","checked_chunks":1,"compressed_checksum_chunks":1,"decoded_chunks":1,"decoded_bytes":11,"original_checksum_verified":true,"container_checksum_status":"verified","dense_line_index_status":"absent","document_index_status":"absent"}
```

### `qzt attest [--level quick|normal|deep] <FILE>`

既定deep。optionはfileの前後どちらでも使えます。検証成功まで何も出さず、成功後に
正準JSON 1行だけを書きます。top-levelの`attestation_schema`は
`qzt-attestation-v1`です。`format: "qzt-0.1"`はQZTコンテナ形式を示します。
他のtop-level fieldは`chunk_count`, `container_checksum`,
`container_id`, `final_file_size`, `format`, `line_count`, `original_checksum`,
`original_size`, `verify`で、nested `verify`は`checked_chunks`,
`compressed_checksum_chunks`, `container_checksum_status`, `decoded_bytes`,
`decoded_chunks`, `dense_line_index_status`, `document_index_status`, `level`,
`original_checksum_verified`を
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
- fieldは`attestation_schema`, `chunk_count`, `container_checksum`, `container_id`,
  `final_file_size`, `format`, `line_count`, `original_checksum`,
  `original_size`, `verify`（上記の検証範囲field）。

#292より前のschema fieldがない出力はlegacy v0です。保存済みの旧byte列とその署名を
組として維持し、旧出力の再生成には[署名guide](guides/attestation.md)記載の旧CLI commitを
固定します。現CLIのv1出力には新しい署名・timestampが必要です。CLI更新だけによる
attestation byte差を、QZT原文の破損と判断しないでください。

実行済み`tests/vectors/valid_c1.qzt.hex` fixture出力:

```json
{"attestation_schema":"qzt-attestation-v1","chunk_count":1,"container_checksum":{"algorithm":"blake3","value":"d05f9357b3182e0e164b508b6cdfd1a2f421559df6886ee2701b330cd5b3a32d"},"container_id":"9885af894b1ee70d8c2cda08e9c68b81","final_file_size":1854,"format":"qzt-0.1","line_count":2,"original_checksum":{"algorithm":"blake3","value":"9885af894b1ee70d8c2cda08e9c68b813aec801465b87a0c16d355d7413b32b7"},"original_size":11,"verify":{"checked_chunks":1,"compressed_checksum_chunks":1,"container_checksum_status":"verified","decoded_bytes":11,"decoded_chunks":1,"dense_line_index_status":"absent","document_index_status":"absent","level":"deep","original_checksum_verified":true}}
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
