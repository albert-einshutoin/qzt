# External Validation: 公開pre.6の実施手順

作者以外の独立した利用者1人が、自身の実データ1種類で公開QZTの主要workflowを
自力完遂できるか確認します。このkitの準備・CI成功はGateの実施・成功を意味しません。

対象は公開[`v0.1.0-pre.6`](https://github.com/albert-einshutoin/qzt/releases/tag/v0.1.0-pre.6)
だけです。製品sourceは `0c8110e6b6e08513b4def3f636b5b79d068e275f`。
参照docsはM1完了commit `5b1a2ea4931041f7b69ec795f24dd00a4a313e89` に固定します。
Rustやsource checkoutは不要です。

- [README: archive導入](https://github.com/albert-einshutoin/qzt/blob/5b1a2ea4931041f7b69ec795f24dd00a4a313e89/README.ja.md#install--インストール)
- [public workflow guide: platformの前提条件と操作](https://github.com/albert-einshutoin/qzt/blob/5b1a2ea4931041f7b69ec795f24dd00a4a313e89/docs/guides/public-workflow.ja.md)
- [CLI reference / C1結果契約](https://github.com/albert-einshutoin/qzt/blob/5b1a2ea4931041f7b69ec795f24dd00a4a313e89/docs/CLI.ja.md#searchcontextverifyの結果判定)
- [pre.6のarchive・binary hashとEngineering検証記録](https://github.com/albert-einshutoin/qzt/blob/5b1a2ea4931041f7b69ec795f24dd00a4a313e89/docs/releases/v0.1.0-pre.6-published.md)

## 実施条件

利用者自身がすべてのcommandを実行します。作者・Codex等による実行代行は行いません。
公開docsの参照は利用できますが、完遂に個別のcommand説明・トラブル解決・結果解釈の
支援が必要だった場合は記録し、支援後も無支援のSUCCESSへ書き換えません。
開発branchのbinary、source build、未公開artifactは使いません。

普段扱うUTF-8ログ・CSV・テキスト等から、本人の実データを1種類選び、変更されない
コピーで実施します。作者のfixtureを代用せず、原本を出力先にしないでください。
原文・path・query・結果全文の共有は任意です。非公開情報は共有版で伏せ、実行した
commandの原記録は本人の手元に残します。記録票の非公開項目には「非公開」と記入します。

## 手順

1. 新しい作業directoryと[記録票](external-validation-report.md)を用意する。
   配布kitのcommitまたはURL、OS/architecture、入力の種類を記録する。
2. READMEからOS/architectureに合う公開archiveと`.sha256`を取得し、checksum照合後に
   展開する。取得URL・archive SHA-256・照合結果を記録し、選択binaryが
   `qzt 0.1.0-pre.6`を返すことを確認する。PATH上の別binaryへ置き換えない。
3. public workflow guideを使い、fixture作成を自身の実データのコピーへ置き換えて
   下表を実行する。`input.txt`はそのコピー、出力は別file。下表はQZTの引数で、
   POSIXではguideの`"$QZT_BIN"`、PowerShellではguideの`Invoke-Qzt`で実行する。

   | 工程 | 引数 |
   |---|---|
   | pack | `pack input.txt -o data.qzt` |
   | 検索準備 | `sidecar-rebuild data.qzt -o data.qzi` |
   | search | `search data.qzt "<検索語>" --sidecar data.qzi --format json` |
   | context | `context data.qzt --offset <hitのlogical_offset> --length <hitのbyte_length> --format json` |
   | Deep verify | `verify data.qzt --deep --format json` |
   | export | `export data.qzt -o restored.txt` |

   原文に該当箇所があるqueryを本人が選び、返されたverified hitを1つ選んでcontextを
   取得する。既定token検索はASCII token向け。日本語・部分文字列は
   [既存ngram手順](https://github.com/albert-einshutoin/qzt/blob/5b1a2ea4931041f7b69ec795f24dd00a4a313e89/docs/guides/search-operations.ja.md#2-tokenとn-gramを使い分ける)
   のindex作成・検索引数を使い、binaryは検証したpre.6のまま、modeとqueryを記録する。
   guideの「23 bytes・1 hit・contextが
   原文全体と一致」はfixture専用で、実データの期待値には使わない。
4. Deep verifyの終了状態と`ok=true`、`level=deep`、
   `original_checksum_verified=true`を確認する。原文コピーとexportをbyte比較する。
   POSIXは`cmp input.txt restored.txt`、Windowsは
   `fc.exe /b "input.txt" "restored.txt"`。比較終了`0`が一致で、非0や未実施は
   一致確認ではない。Windowsの`1`は不一致、`2`は比較エラーです。
   [fc.exeの説明](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/fc)
5. 各工程のcommand・終了状態・結果・概算時間と、結果の解釈を本人の言葉で記録する。
   非公開データでも比較方法・終了コード・本人の一致確認結果は残す。
   失敗、再試行、途中離脱、必要だった支援を削除せず追記する。

## C1の読み方と任意の補助確認

返したhitの`source=verified_original_bytes`は、そのhitが原文byteで検索条件を
満たした保証です。検索網羅性の保証ではありません。`index_coverage_verified=false`、
complete宣言、capなし、通常のzero hitのいずれも、全件取得や原文での不存在を証明しません。
`capped`・停止理由・`incomplete_reason`、contextのscope・両側のstop・fragmentを確認します。
partialを要求した範囲の完全取得として扱わず、その結果だけで要求完遂と判定しません。
`scope_boundary`や、単一fileのpackで`no_document_index`になること自体は異常ではありません。
contextは原文の抜粋で、`text_escaped`は表示用です。Deep verifyはQZIを検証しません。
正確なcontext byteの確認には`excerpt.bytes_hex`を使います。必要fieldの欠落・未知の
statusや終了非0を成功と扱わず、観測内容を記録します。

可能なら、選んだ検索modeの条件で原文に該当しないqueryを1つ試し、command・状態と
「zero hitから何が言え、何が言えないか」を本人の言葉で記録します。
この補助確認は任意で、未実施だけを理由にSUCCESSを否定しません。
実際に観測していない破損・partialの扱いは理解確認として記録できるものの、
異常系を実行検証した証拠とはしません。

## 最終判定

| 判定 | 基準 |
|---|---|
| SUCCESS | 公開pre.6と本人の実データで、個別支援・作者代行なしに全工程を完遂し、Deep verify・export byte一致・C1の保証範囲を確認できた。 |
| NEEDS_SUPPORT | 個別のcommand説明・トラブル解決・結果解釈の支援が完遂に必要だった。支援後の完遂でもSUCCESSへ書き換えない。 |
| PRODUCT_FAILURE | 正しいartifact・入力・操作条件を確認でき、byte不一致、検索modeの契約に反するhit/context、破損の正常扱い等、製品側の問題を観測した。証拠と条件を残す。 |
| INCOMPLETE | 時間切れ・途中離脱・本人データ未使用・公開物以外の使用・作者代行・必要な証拠不足等で、Gateを評価できない。 |

評価の前提が成立しない回はINCOMPLETEとし、未確認の原因をPRODUCT_FAILUREと推測しません。
有効な実施で製品側の問題が観測された場合はPRODUCT_FAILUREを優先し、支援の有無も残します。
製品側の問題がなければ、必要だった個別支援、全工程と理解の達成、残る未完了の順に判定します。
破損・未対応入力の正しい拒否、通常のcap・partial・coverage unknownは、それだけで
PRODUCT_FAILUREにしません。要求を満たすhit/contextを取得できないまま終了した場合は
INCOMPLETE、解決に個別支援が必要だった場合はNEEDS_SUPPORTです。

所要時間は本人による概算で十分です。自動収集・telemetry・benchmark計測は行いません。
このGateは検索網羅性、G3のresource envelope、G4の競合性能、buildのbit-for-bit再現性、
他利用者・他データへの一般化を証明しません。Gate終了後の観測分類は記録票にあります。
