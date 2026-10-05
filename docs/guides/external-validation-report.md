# External Validation: 実施結果記録票

[実施手順・判定基準](external-validation.md)と一緒に配布します。
未確認は「未確認」、共有しない情報は「非公開」、任意確認をしない場合は「未実施」と
記入します。成功・失敗・支援・離脱・再試行を削除せず、本人の説明を記録した後に判定します。
この空の記録票は、External Validationの実施証拠ではありません。

## 実施条件と入力

- 実施ID / 日時：
- 配布kitのcommitまたはURL：
- 参加者ID / 作者以外の利用者であることの確認：
- QZT利用経験：
- OS / version / architecture：
- 公開archive名 / 取得URL：
- archive SHA-256 / sidecarとの照合結果：
- binary version / 選択binaryのpath（共有時は伏せて可）：
- 本人の実データの種類 / 普段の用途：
- 入力は実施中に変更されないコピーか：
- 公開可能なサイズ / 行数 / 入力hash（非公開・未計測も可）：
- 検索mode / query（非公開も可）：
- 作者・Codex等による操作代行の有無 / 内容：

## 本人の実行記録

時間は概算でよく、benchmark精度や自動計測は不要です。原文・query・結果全文を
提出する必要はありません。共有版で伏せた箇所を明示し、実際のcommandは本人の手元に残します。

| 工程 | 実行したcommand | 終了コード・結果 | 概算所要時間 |
|---|---|---|---|
| install / checksum / version | | | |
| pack | | | |
| sidecar作成 | | | |
| search | | | |
| context | | | |
| Deep verify | | | |
| export | | | |
| byte比較 | | | |

- search：選んだhitの`source` / `logical_offset` / `byte_length`：
- search：`capped` / `stop_reason` / `incomplete_reason`：
- search：`index_complete_declared` / `index_coverage_verified`：
- 本人の説明：hitの正しさと検索網羅性について、何が分かり何が分からないか：
- context：`mapping_status` / scope / `before.stop` / `after.stop`：
- context：`leading_fragment` / `trailing_fragment` / 原文との見比べ結果：
- 本人の説明：抜粋の範囲と停止理由をどう解釈したか：
- Deep verify：`ok` / `level` / `original_checksum_verified`：
- 本人の説明：Deep verifyがQZI・検索網羅性も保証すると思うか、その理由：
- byte比較：比較方法 / 終了コード / 本人による一致・不一致・未確認の結果：
- 非公開データの原文一致確認：本人が上記方法で確認した結果：

## 任意のzero-hit補助確認

未実施でもGate成功の必須条件には影響しません。

- 実施 / 未実施：
- 選んだmodeの条件で原文に該当しないと判断したquery / 確認方法（非公開も可）：
- 実行したcommand / 終了コード / hit数 / cap・理由・coverageの状態：
- 本人の説明：この結果は検索網羅性や原文での不存在を証明するか、その理由：
- 破損・partialが未観測の場合の理解確認（実行証拠とは区別する）：

## UX・支援・継続利用

- 総所要時間（概算。読む・調べる・再試行の時間も含む）：
- 迷った箇所 / 理解できなかった用語：
- 必要だった支援（誰から・何を・いつ。必要だが得られなかった支援も記録）：
- 支援を受けた場合、その後どこまで完遂したか：
- 途中離脱した工程 / 理由：
- 継続して使いたいか（はい / 条件付き / いいえ） / 理由：
- 使わない場合の理由：

## 失敗・再試行履歴

失敗や支援後の完遂を消さず追記します。支援が必要だった実施をSUCCESSへ書き換えません。

| attempt | 工程・現象 | 対応・必要だった支援 | 結果・本人の説明 |
|---|---|---|---|
| | | | |

## 最終判定と観測事項

- 最終判定（SUCCESS / NEEDS_SUPPORT / PRODUCT_FAILURE / INCOMPLETEの1つ）：
- 判定者 / 日時 / [基準](external-validation.md#最終判定)に照らした根拠：
- 公開pre.6のみ / 本人データ / 本人の操作を確認できたか：
- 必須工程の完遂 / Deep verify / export byte一致を確認できたか：
- verified hitと網羅性、partial・破損の扱いを正しく理解できたか：
- 製品側の問題を観測した場合、期待・実際・入力と操作条件・証拠：
- 評価できない条件・未完了・証拠不足がある場合、その内容：

Gate終了後にIssue候補を整理する際は、下表を使います。観測と原因仮説を分け、
既存Issueとの重複と再現条件を確認してからIssue化を判断します。このkitによる自動Issue作成はありません。

| 分類 | 観測例・整理方法 |
|---|---|
| 正しさ・データ保全 | byte不一致、誤hit/context、破損の正常扱い。最優先で入力・操作条件と証拠を整理する。 |
| 結果契約・理解 | coverage・partial・verify対象の誤解。説明不足、表示の分かりにくさ、契約との不一致を区別する。 |
| 導入・workflow UX | artifact選択、checksum、path、前提ツール、手順で停止した箇所を残す。 |
| 入力・運用制約 | encoding、行長、resource error、待ち時間等。既知制約・未対応入力・予期しない失敗を区別し、G3/G4は将来の測定候補に留める。 |
| 採用価値 | 完遂しても使わない理由や用途との不一致。直ちに新機能Issueへ変換しない。 |

- 観測ID / 分類 / 工程：
- 期待 / 実際 / 本人の言葉：
- 必要だった支援 / 再現可能性 / 公開できる証拠：
- 原因仮説（未確認なら未確認） / 既存Issueとの重複：
