# 1 GiB 実文書評価（2026-10-26）

## 確認できた規模と条件

原本文1,181,143,804 bytes、23,743実記事で単体QZTのpack/deep verifyと全量復元が成功した。
単体の既定token QZIは `a resource limit was exceeded`、exit 1で拒否された。
失敗QZIは残らず、予算を増やして再試行していない。これは固定binary・固定入力の再現可能な制約であり、
全QZT利用が1 GiBで不可能という意味ではない。拒否を生んだ内部limitの個別特定はしていない。
事前校正で固定した8 MiB上限・文書境界・page ID順の142分割は、全pack/verify/index/inspectが成功した。

元は英語Wikipedia `20261001` dump jobの最初の2 archive。XMLからnamespace 0、非redirect、wikitextの
非空本文だけを抽出し、exact raw SHA256で重複を除去した。旧9文書や合成paddingは含まない。
除外はnamespace/redirect 7,253件、exact重複0件、4 MiB超0件。末尾LFを足した正規化後は1,181,167,547 bytes。
文書長は最小48、中央値30,457、p90 123,775、最大658,336 bytes。
dump jobの日付は全記事のrevision日時や公開完了日ではない。固定質問の概要にある「2026-10-01公開dump」は
job識別日を指す表現で、各revisionの実日時はinventoryを正本とする。現在の研究全体を表すsnapshotとは扱わない。

sourceは `1562940cf9a93e885d32faf0f838a5b288d96497`、native binary SHA256は
`53778d7b266dd69e2e93101ca6abd4944cac30236fca347fa2c095881117ef4f`。
全量復元1,181,167,547 bytesのSHA256は元の連結本文と同じ
`a945452eb8dc4b9b0f31ebdb36786f25eb1373fd8f1f3b7cde0089c511c8309b`。
Core/API/format/既定予算、依存、CI、releaseは変更していない。
Document Indexを使うpack-docsではなくstreaming packと共通外部inventoryによる配置である。

## 機械計測

Apple M4/arm64、memory 32 GiB、macOS 27.0、外付けSatechi上。Rust/Cargo 1.96.0、rg 15.1.0。
測定スクリプトのPythonは3.9.6、検証gateは別途記録する。cacheをpurgeせずwarm/unknown。
固定機械計測中は回答Agentを動かさず、同一hostで逐次実行した。
CLI実時間は `/usr/bin/time -l` の丸めたrealとwrapper wallを分ける。RSS samplingはOS強制上限ではない。

| 構築段階 | native real合計 秒 | 監視wrapper wall合計 秒 | 最大native RSS bytes |
| --- | ---: | ---: | ---: |
| 単体pack | 7.29 | 7.324 | 29,802,496 |
| 単体deep verify | 3.39 | 3.659 | 12,402,688 |
| 単体既定index拒否 | 0.88 | 1.043 | 93,847,552 |
| 142分割pack | 12.25 | 50.875 | 22,675,456 |
| 142分割deep verify | 5.75 | 50.822 | 12,140,544 |
| 142分割index | 33.19 | 50.525 | 54,755,328 |
| 142分割inspect | 2.79 | 48.366 | 16,236,544 |

準備orchestration wallは213.525秒。0.25秒ごとの監視待ちが短いcommandへ大きく加わるので、
native realと同一視しない。元資料取得は共通費用で、初回153.071秒でhost pressure warning継続により停止、
回復後の明示的な1回retryは189.653秒で成功した。初回子family RSSは約38 MiBで、QZTによる規模拒否ではない。
取得済みarchive1を一次SHA1で再検証して再利用し、通信量は旧partialと未flushの保守上限を引き継いだ。
compiler buildはCargo logで8.62秒、peak RSSは未計測。校正・コピー・inventory生成・開始終了hash・失敗試行も
実験費用として残すが、layout全体のwallを独立計測しておらず、213.525秒を全工程の総費用とは呼ばない。
全量exportの追加整合性検証はnative 2.41秒、wrapper 2.659秒、SHA照合0.450秒。検索計測には加算しない。

全体検索はBがrg whole-word literal、Cがtoken単語の同一行一致。Cは全142 QZI/QZTを毎回開く。
両方が共通表示上限12文書で打ち切る。Bは表示用の実一致offset取得にも12 CLIを使い、合計13起動、
Cは142検索と12 rangeの合計154起動。queryの意味と網羅性が異なるため、同等の完全検索とは呼べない。

| query | B 秒（3回順） | C 秒（3回順） | Cのpartial shard数（各回同じ） |
| --- | --- | --- | ---: |
| Rutherford | 10.723 / 10.075 / 1.550 | 5.453 / 4.117 / 6.040 | 47 |
| entropy | 10.193 / 9.931 / 0.622 | 5.070 / 5.187 / 3.901 | 47 |
| adenine | 9.863 / 10.027 / 0.564 | 4.386 / 4.386 / 3.318 | 10 |
| Turing | 9.915 / 10.100 / 0.551 | 5.057 / 5.068 / 3.828 | 56 |
| identity | 9.794 / 9.820 / 0.600 | 4.957 / 5.115 / 3.907 | 142 |

Cのpartialは主に各shardの8結果上限による。全分割へ問い合わせたことは、全結果を返したことを意味しない。
最初の2回にCのwallが短いqueryがある一方、3回目はBが全queryで短い。cache/時系列変動の大きい観測である。
native検索が報告したphysical decoded bytesは最初の各queryで約241〜813 MB、posting bytesは334〜13,453。
これはOS I/O bytesではなくnative内部counterで、表示用rangeの費用はwall/CLI数側に含まれる。

既知参照は回答探索から独立させ、全100文書rankへ分散した最大512-byte断片からseedで順序を固定した。
UTF-8の両端で切れた文字bytesは除き、実際のoffset/lengthを固定した。
同一点と分散を各1/10/100回。Bは平文range、Cは同じdoc/offsetのrange、単体QZTもindexなしで測った。
全666取得（3条件×2系列×111回）は元fragment SHAと一致した。以下は同じ1系列を各1回測った値で、反復分布ではない。

| 条件 | 同一点1 / 10 / 100回 秒 | 分散1 / 10 / 100回 秒 |
| --- | --- | --- |
| B | 0.000056 / 0.000234 / 0.002019 | 0.000037 / 0.000332 / 0.003060 |
| C・142分割 | 0.011410 / 0.111690 / 1.139437 | 0.011509 / 0.116018 / 1.137136 |
| C・単体、indexなし | 0.014390 / 0.111551 / 1.125833 | 0.011494 / 0.130871 / 1.288584 |

このhostで既知参照の平文アクセスは大幅に短い。QZTにはCLI起動と必要chunkの復元がある。
取得量・復元量・OS I/Oを混同せず、QZT rangeが全量exportより少ない要求で実行できたことと、平文より速いかを分ける。

## 保存費用

以下は論理file bytesで、filesystem圧縮・clone共有や実OS I/Oの測定ではない。
共通metadataはinventory 11,319,137＋configuration 29,048＝11,348,185 bytes。

| 運用データ | bytes |
| --- | ---: |
| B・平文＋共通metadata | 1,192,515,732 |
| C・142 QZT | 396,364,699 |
| C・142 QZI | 472,400,489 |
| C・142 QZT/QZI＋共通metadata | 880,113,373 |
| C・単体QZT＋共通metadata、indexなし | 407,499,116 |
| 原文も維持するC・142分割＋共通metadata | 2,061,280,920 |

検索可能な分割CはBより約26.2%小さい。単体QZTのみの圧縮率を検索運用全体の削減率には使わない。
原文を保持する場合はCが約868.8 MBの追加となる。QZT binary 1,706,656 bytes、rg等のtool install、
校正、原本archive、旧partial、連結・分割平文、全量復元検証用copy、全文traceはこの最小運用表とは別の実験保管費用である。
2026-10-26の観測時点の実験directory全体は論理8,552,750,497 bytes。canonical corpus約1.900 GB、
旧中断corpus約1.612 GB、校正約0.201 GB、material約2.760 GB、分割平文約1.181 GB、分割QZT/QZI約0.869 GB、
残りはtrace/metadata等である。保管費用を最小運用表から消しておらず、`storage-observation.json` に内訳を残した。

## 回答・独立採点

8 fresh回答と4 fresh匿名採点をすべて実施した。同じgpt-6.1-sol/medium、fork none、各900秒。
旧評価の回答者・採点者contextを再利用していない。開始/入力/提出SHA、実際のcanonical Agent名とlaunch call IDは
`evidence/agent-launches.json`、`responder-runs.json`、`judge-runs.json` に記録した。
主評価12体に加え、setup中断3体と専門レビュー1体の起動も隠さず残す。backend UUIDはAPIから得られていない。

全80回答が期限内・schema適合・調査済みで提出された。192引用は原文、byte範囲、全文SHA、各自の成功した
取得範囲に一致する。開始/終了の全23,743原文、binary、QZT/QZI、inventory、連結/分割入力の照合が成功した。
終了時hash計測14.432秒は別instrumentationである。引用が実在することと、主張を十分支えることは別である。

| 試行 | 提出wall 秒 | 操作数 | gateway返却 bytes | native CLI数 | native wrapper 秒 |
| --- | ---: | ---: | ---: | ---: | ---: |
| B1 | 330.4 | 52 | 133,145 | 0 | 0.000 |
| B2 | 333.1 | 52 | 140,777 | 0 | 0.000 |
| B3 | 349.6 | 58 | 145,434 | 0 | 0.000 |
| B4 | 281.8 | 54 | 111,610 | 0 | 0.000 |
| C1 | 341.1 | 56 | 141,915 | 22 | 0.343 |
| C2 | 315.8 | 52 | 160,704 | 20 | 0.389 |
| C3 | 311.8 | 54 | 122,371 | 22 | 0.406 |
| C4 | 267.1 | 49 | 135,087 | 18 | 0.319 |

全8体が `find/read/within` を選び、全体searchを一度も呼んでいない。共通title inventoryで資料を特定できる
既知概念の課題であり、Cの回答成立はQZI検索品質の証明ではない。単体QZTの回答評価はしていない。
Bの読取はPython、CはQZT range、その後の文書内literal検索は共通Python。inventory再読取は各試行計2.14〜3.31秒、
lock queueは各0〜0.181秒。model推論、操作選択、通信、共有host、親のmetadata処理/Python検証を含むwallを
製品速度と呼ばない。model token数・課金は未取得でunknown。返却bytesをtokenや課金に比例換算していない。

匿名bundle 334,132 bytesとそのSHAを固定し、同一内容をJ1〜J4へ渡した。明示的な条件・model labelを含めず、
元回答の文言・引用は改変していない。回答の文体等からの推測不能を保証するものではない。
各Jは80行を期限内提出。per-read tool traceはgatewayからexportしておらず、固定入力と採点出力を保存した。

| 採点者 | B 事実/80 | C 事実/80 | B 引用/80 | C 引用/80 | B/C 完遂/80 | B/C 追加根拠不足件数 |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| J1 | 79 | 79 | 75 | 73 | 79 / 79 | 6 / 4 |
| J2 | 75 | 76 | 79 | 79 | 75 / 76 | 6 / 5 |
| J3 | 79 | 80 | 79 | 79 | 79 / 80 | 6 / 5 |
| J4 | 78 | 75 | 73 | 70 | 78 / 75 | 6 / 5 |

全Jの条件保持はB/C各80/80、q10の適切な回答不能は各4/4、調査は各40/40、原文に反する断定の指摘は0。
q10の提出statusは6体がunknown、2体がansweredだが、本文はいずれも実測百分率を確定不能と説明している。
8次元での行単位完全一致は61/80、19行は差異がある。これは人間の採点一致率ではない。

親は多数決にせず固定原文とrubricへ戻った。q1は全8体で分散電荷/電場強度という固定主要点が明示されず、
B4/q2はBorn則の振幅から確率への関係、C3/q7は有用仕事を制約/定量する役割が欠けると裁定した。
この保守的な事実・完遂の裁定はB/Cとも75/80。q1の指示語・図の引用が主張をどこまで直接支えるかは
全8行で未解決とし、citationはnull/unknownのまま保存した。親の引用総合点は計算していない。
q9ではB3だけが定数の役割を定義する冒頭を引用し、他7体は自然対数底/円周直径比の直接引用を欠くので引用1。
B1/C3のq2の測定後規則、B2/C2/C3のq7の仕事に関する引用支持も部分的である。

追加根拠不足として挙がった11回答内の4命題（DNA polymerase、Lorentzian geometry、外見不変の進化、
表面重力と温度の対応）は、元本文と当該回答者が実際に取得したfragmentにはすべて実在した。
固定judge抜粋の外だったため、複数Jが同じ不足を共有した。`adjudication-source-grounds.json` に
独立の原文range/SHAと取得確認を追加し、元採点は保存した。抜粋で確認不能と原文に根拠がないことを混同しない。
全断定の世界的真偽を証明したという意味でもない。未解決の引用解釈はunknownのままである。

全8体のq10は共通概要の「2026-10-01公開dump」という不正確な表現を引き継いだ。
これはB/C共通の刺激の制約で、公開日・記事revision日時を確認した根拠として採用しない。
回答や固定rubricを事後修正せず、raw採点と親のprovenance上の訂正を分離した。

## 失敗と再現上の制約

最初の回答開始時、asset sealがQZT binaryを0444にして起動不能にした。開始済みC1/B1/B2を一律中断し、
回答・イベント・旧freezeと旧コードを `aborted-setup-permissions` に保存した。結果を有効試行へ移していない。
binaryのみ0555、他0444とする1行修正に、修正前PermissionErrorを再現する回帰チェックを追加した。
修正後native E2E込み12件成功、同じ専門担当が当該修正だけ静的Approve。全8体を新規contextで再開始した。
旧/新freezeの差はfreeze.pyとtest_scale.pyだけで、質問・rubric・query・配置・binary bytesは同一。
構築と機械計測は旧seal前に完了していたため再計測せず旧freezeとの対応を保持した。

最終Companion初回GateはP2 2件でBLOCKだった。再現コードが宣言binary SHAを実行時に要求しない点と、
単体indexの任意非0を受理する点である。実測binaryのSHAと実際のexit 1/resource-limit診断は元記録で確認できるが、
再実行契約もfail-closedにするため、開始/成功保存前/封印時のSHA照合と、既知診断への限定を追加した。
exit 0や権限/内部エラー・空診断も、固定条件との差として停止する。正しいresource-limitのときだけ分割条件へ進む。
修正前の検出と修正後native E2E込み17件成功を保存し、同じ2指摘の最終再確認を行う。
1 GiB構築や拒否を再実行せず、実測時の8スクリプトをhash一致するsnapshotへ保存した。
既存のmeasured freeze・結果・回答・採点は改変していない。現行コードと実測コードの差を隠さず、
`measured-harness-source.json.gz` と補強receiptを分離する。

原本文はlocal-only保管で、Gitにはsource inventory、hash、集計、引用を含む小さい回答・採点を残す。
公開dumpの将来保持は保証しない。rawがない状態の第三者は全文traceを完全再監査できない。
指示によるAgent資料分離でありOS sandboxではない。これは研究者全員の合意や実利用者の需要の調査ではない。

## 検証と判断

今回の判断は**現状維持**とする。1 GiBでのseekableな圧縮保存と既知参照復元は成立したが、
平文より速い既知参照、網羅性を揃えた検索優位、回答品質の改善は示されなかった。
QZTの有効領域は容量を重視するcold evidenceと参照位置が分かる復元に限定して説明できる。
既定単体indexの規模拒否は今後の制約だが、今回の回答者はその検索を使わず、予算引上げや新しい検索基盤を
直ちに実装する需要根拠にはならない。旧小規模評価の判断をこの評価へ転用せず、今回も拡張実装は保留する。

同等coverageでの定常検索時間差が正に測れた場合の損益分岐モデルは
`ceil((C追加準備秒)/(B反復秒-C反復秒))` だが、本測定は検索意味/上限/coverageとcacheが揃わず実測Nはunknown。
既知参照ではCが遅く、latency上の有限の損益分岐は本観測から得られない。保存容量の便益とは別の判断である。
四つの同一model試行と科学の既知概念10問は小さく偏った標本であり、一般の巨大資料探索、全利用者や
人間の需要に一般化しない。新しい実装・Issue・次Goalは開始しない。

実行した検証:

- native binaryを指定した `python3 -m unittest discover -s examples/gib-scale-evaluation -p test_scale.py`: 最終17件成功。実測前のpermission修正時は12件成功。
- `QZT_GIB_BINARY=... /opt/homebrew/bin/python3.14 -m unittest discover -s scripts -p 'test_*.py'`: 最終58件中57成功、旧Agent評価のprepared runtime要件による1 skip。今回のnative E2Eは実行済み。
- `make check`: fmt/clippy/default build/all-target all-feature tests/docを含め成功。
- `cargo package --allow-dirty`: 成功。release/tag/publishを行うcommandではない。
- 新規コード・公開evidenceのgitleaks、最終Companion、最終HEAD/main CI/securityは納品receiptへ記録する。

未実施はcold cache制御、OS I/O bytes計測、全モデルtoken/料金計測、他hostでの1 GiB再測定、人間評価、実利用需要、
全source/全科学理論の世界的妥当性の検証、単体QZTでの回答試行、同等網羅性での検索比較である。
Gitには巨大な原本・native stdout・QZT/QZI・binaryを追加せず、再取得コードと小さいevidenceを納品する。
