# 実行結果（2026-10-12）

保存snapshot（10文書、36,633 bytes）から、repo外のfresh directoryでproducer/recipient/evaluateを実行した。採用証拠は[canonical index](canonical-index.json)から[raw/canonical-final-20261012](raw/canonical-final-20261012/)へ辿れる。3工程ともexit 0、検索hitから7文書を復元した。残り3文書は検索根拠として復元していない。

資料commitは `3d092d1db64fdd3f0744377df98257038eddd32f`。製品sourceは `c11b7eb9a025f75d84ea9d3029a0674a606c04c6`。`build-receipt.json`は、同sourceとのCore/Cargo差分なし、27 source fileのSHA-256、rustc/cargo version、`cargo build --locked --bin qzt`のcommand/exitを記録する。保護binaryのSHA-256は `a7696eb45b9d41a8714d83aac5b2d0f4a839908cd4d734801bf0055fb8be3d0a`。producer引数のsource SHA自体は自己申告であり、このbuild記録と分ける。

`run-freeze.json`に全実行helper、test wrapper、質問・評価、snapshot/policy、binary/build receiptのsize/SHAをpack前に保存し、実行後にも不変を確認した。文書ごとの出典・取得時刻・公開更新時刻またはcommit時刻・object ID・抽出pointer・原文hashは`handoff-receipt.json`とmanifestにある。実command/exitは`orchestration.json`、producer/recipient logsに記録し、manifest/reportの外部期待SHAも評価器へ渡した。これは署名による認証ではない。

q1〜q6 は固定した各 fragment と document の byte correspondence を取得した。q5 は `large benchmark matrix` の review と `明示env budget` の返信という、指定された2文書の両方を取得した。q7 は hit 0 で `record_insufficient` とし、不存在の証明にはしていない。全問で `index_coverage_verified=false` のままである。

| 問い | 固定評価 |
| --- | --- |
| q1 導入理由 | acquired。100 MiB/ngramを外部監視で中止し成功時間/RSSが不明だったため、製品自身が構築超過前に拒否する契約を導入した記録 |
| q2 10 MiBの制約 | acquired。8M postingの新既定でC2/C4 10 MiB/ngramを拒否する受理範囲縮小。100 MiB完走や性能改善率を受入条件にしていない |
| q3 16M確認 | acquired。明示posting16Mの10 MiB限定確認でbefore QZI bytes・検索結果が一致。100 MiBの完走・成功peak・性能改善は未確認 |
| q4 超過時保護 | acquired。入力QZTと既存QZIを保持し、部分index成功やraw-scan fallbackを作らない契約 |
| q5 指摘→対応 | acquired。review `4238230444`のmatrix指摘とreply `4238290457`の対応を別文書で取得。明示env予算を伝播し、資源拒否は`measured_success=false`、他errorはpanic。自動増額・100 MB matrix再実行は行わない記録 |
| q6 mainと公開版 | acquired。当時のmain納品記録と公開pre.6に新optionがない記録を区別。現在状態をsnapshotから推測しない |
| q7 実運用性能・実際の合意 | record_insufficient。0 hitは不存在の証明ではない。実際の合意・未記録の実運用性能は判定できない |

検索の cap、known incomplete、`index_complete_declared=false` は tool failure と混同せず `partial_unknown`（reason/query/raw metadata付き）にする。未知schema・未知reason・不正bytes/range は Error である。

report v4はhit文書だけの`verified_documents`を持つ。evaluateはmanifest、pre-pack receipt、
questions/evaluation hashへ結び、文書full bytesを再hashしてから全query/raw hit/contextを
同じvalidatorで照合する。context budget/fragmentを含むknown partialは全7問の一覧を
止めず`partial_unknown`として取得済み/未取得fragmentと診断を残す。外部出典の真正性、
意味、同意はこのbyte proofの保証外である。

この表は固定根拠fragmentと検証済み原文bytesの対応を人が説明したもの。回答の自動意味判定、一般検索品質、LLM精度向上の結果ではない。

初回 attempt は q5 が missing だったため成功へ再分類しない。後変更 trial も採用しない。ユーザーが 2026-10-12 に許可した q5 の検索語・正解根拠訂正後の新 trial だけをこの結果に用いた。initial evaluation を元 producer/evaluation hash と完全一致する形で再構成できる証拠はないため、初回評価本文の完全復元は unknown とする。

検証では raw snapshot/body改ざん、snapshot外path/親symlink、policy改ざん、manifest range改ざんを更新済み外部sealで渡すケース、質問改ざん、QZT/QZI破損、原文/文書digest不一致、query/hit/検索記録の改変、未知schema/reason、known cap/incomplete/context fragment、既存receipt/output/logの拒否・保護を確認した。取得器の保存response再生→producer受理もネットワーク不要で確認し、実データPython回帰10件と次の両wrapperが成功した。

```sh
cargo test --locked --test phase61_public_history_evidence --test phase60_decision_evidence
```

Sol medium専門レビュー1担当の指摘を共有検証経路と取得schema正本で修正し、
同担当の指摘範囲の最終判定は[Approve](raw/review/specialist.ja.md)。
fresh Companion Stop Review Gate（Luna medium）は[ALLOW、touchedFiles=[]](raw/review/companion.json)。
両者は静的判定であり、runtime再検証や最終HEAD/mainのhosted CI、merge、Issue完了を含まない。

これは mutable な Issue/PR snapshot と fixed commit 資料を区別した限定的実演である。report/source の署名認証、意味判断、coverage、第三者検証、production、release、現在の main 状態は証明しない。
