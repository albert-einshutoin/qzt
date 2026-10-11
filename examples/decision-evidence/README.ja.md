# 要約から失われた判断を原文参照で回復する

人物・日時・承認・試験は全て架空の「灯台ノート」開発記録。7文書の理由・反対・条件・
変更経緯を短い要約は落としている。8問と指定根拠、意味解釈用oracleはpack前に固定した。
受取側は回答を生成せず、6問の指定原文byteを検証して示し、2問をunknownのまま残す。

| 問い | 要約だけ | 指定原文から回復できること |
| --- | --- | --- |
| q1 Aの理由/Bの利点・懸念 | 初めにAを選んだ | 01:既存運用による保守性、Aの通信断とBの復旧時重複・順序の懸念 |
| q2 Bの保留理由 | 不明 | 02:重複排除・復旧試験の記録待ち。結果が残れば再検討 |
| q3 Aの条件 | 不明 | 04:一日100件以下、短時間通信断、翌日までの欠落確認 |
| q4 反対意見 | 不明 | 03:後日の書き直しと当日記録の混同への反対。この記録に撤回なし |
| q5 変更根拠 | Bに変更した | 05/06:一日300件・最大72時間断という計画変更、架空の復旧試験記録、Aの撤回 |
| q6 記録上の最新判断 | Bに変更した | 07:Bの条件付き限定試行、整合確認・欠落時停止。本採用は未決定 |
| q7 実際の全員合意 | 確認不可 | unknown。参照manifestが根拠未提示と分類。真の不存在や実際の合意を判定しない |
| q8 未記録の応答時間測定値 | 確認不可 | unknown。計画件数/停止条件を性能測定値へ読み替えない |

## 再実行

Python 3.12以降、Cargo/Rustと通常のビルド環境が前提。repository rootで実行する。
base product sourceは`3d092d1db64fdd3f0744377df98257038eddd32f`。
公開pre.6 source `0c8110e6b6e08513b4def3f636b5b79d068e275f`とは別であり、
version文字列が同じでもbinary同一を意味しない。生成時のbinary SHA-256をproducer記録へ残す。
source checkoutはmain納品後のこの例を含むrevisionを使う。公開binaryでの結果とは扱わない。

```sh
cargo build --locked --bin qzt
qzt_decision_workspace=$(mktemp -d)
python3 examples/decision-evidence/produce.py \
  --binary "$PWD/target/debug/qzt" \
  --fixture "$PWD/examples/decision-evidence/fixture" \
  --output "$qzt_decision_workspace/recipient"
```

producerのstdoutはmanifest SHA-256。表示した値を信頼済み経路で受取側へ伝える。
QZTのbytesとは別の期待値であり、受取側がmanifest自身からhashを作って認証したことにしない。
producerは原文由来の範囲・SHA-256・query座標をpack前に隣のproducer.jsonlへ保存する。
Document Index付き`pack-docs`とexact export照合の後、handoff directoryへ次の5 fileだけを渡す。

- evidence.qzt
- summary.txt
- manifest.json（unsigned、原文・回答を含まない参照/質問データ）
- qzt（Windowsではqzt.exe）
- recipient.py（標準ライブラリだけ。fixture/oracleのimportなし）

受取側はhandoffの中で次を実行する。`<信頼済みmanifest SHA-256>`をproducer表示値に置き換える。

```sh
cd "$qzt_decision_workspace/recipient"
python3 recipient.py --manifest-sha256 '<信頼済みmanifest SHA-256>' \
  --output report.json --log commands.json
```

`report.json`に各問の原文、doc_id、logical_offset/byte_length、checksum検証結果が出る。
これは回答生成/意味理解の結果ではない。判断の解釈は上の固定表と原文を人が確認する。
recipientはcwdの固定名だけを読み、repo平文のpathやoracleを知らない。テストは5 file inventoryと
独立fixtureのbyte oracleを外から照合する。これは入力分離の実演で、OS sandboxや独立第三者実行の証明ではない。

検証はcontainer_id、Document Indexのdoc_id/範囲、Deep verify、`doc`のBLAKE3検証、
`range`の同じbytes、信頼済みmanifestの原文由来SHA-256を組み合わせる。
期待checksumを復元結果から作らない。QZT Coreの既存API/形式を変更しない。

q5は既定構築予算内でngram QZIを生成し、`前提変更：`を検索する。返されたhitのbyteと
期待座標を検査し、そのhitから`context`を取得する。文書帰属・local/global範囲・hex bytesを
検証済み文書と照合する。QZIは派生物なので、存在しない時だけ再構築し、既存QZIの破損を
黙って別経路へfallbackしない。cap/partial/incomplete/未知の保証fieldは、この所定の確認をErrorにする。
coverageはfalse/unknownとして残す。1 hitや0件は一般的な網羅性/不存在を証明しない。

## 要約を変えても根拠は不変

この段階でQZT/manifestと旧report/logを保持し、summaryだけを更新できる。
評価者はfixture/summary-updated.txtをsummary.txtへcopyして同じmanifest期待hashで実行する。

```sh
python3 recipient.py --manifest-sha256 '<同じ信頼済みmanifest SHA-256>' \
  --output report-updated.json --log commands-updated.json
```

同じoutput/log名への再実行はexit1で拒否し、既存fileを変更しない。producerも既存directoryを拒否する。
新しい要約から期待checksum/原文/参照を作り直さない。テストではQZI削除後の再構築も照合する。

```sh
# repository rootで実行
cargo test --locked --features internal-testing --test phase60_decision_evidence --test phase21_evidence_retrieval
# PythonのE2Eだけを実行する場合
QZT_DECISION_BINARY="$PWD/target/debug/qzt" \
  python3 -m unittest discover -s examples/decision-evidence -p test_case.py
```

各CLIは30秒timeout。fixture合計64KiB以下、単一実行。全テストにはPythonが必要。
make check/packageは最も近いAGENTS.mdの完全gate、profile/過去大規模benchmarkは実行しない。

## 信頼と限界

checksum一致は、信頼して渡された期待値とのbyte一致だけを示す。unsigned manifestと
実行物の信頼は送り手/受取側の運用前提で、自己計算hashは本人性・真実性・時刻・合意を証明しない。
manifestと期待hashを攻撃者が同時に置換できる場合、この例はその偽造を認証で防げない。
要約は独立して更新できる非権威的な表示資料。元の条件付き採用・保留・反対・撤回を
無条件の承認へ変換しない。過去の架空承認を現在の実行許可として使わない。

記録不足はこの固定ケースの事前分類unknown。不正参照、破損、必要な根拠の欠落はError。
推測・別文書・全体検索への暗黙fallbackで埋めない。最新判断は指定した7件の範囲でのみ読む。
LLM精度向上、実際の合意、独立第三者検証、production-ready、MemoryPagerの実装を主張しない。
