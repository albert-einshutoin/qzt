# Sol medium専門レビュー（1担当）

Issue #348、base `c11b7eb9a025f75d84ea9d3029a0674a606c04c6`。
担当 `evidence_specialist`、model `gpt-5.6-sol`、effort `medium`。
全判定READ_ONLY / STATIC_ONLY。編集、code実行/import/test、network、subagentなし。
runtime、hosted CI/security、Companion、PR/mergeの成功を含めない。

## 最初の判定: Request changes

直接の受入指摘:

1. snapshot memberの絶対path/`..`/親symlinkとAPI body未照合。read前のcontainmentとraw/body一致が必要。
2. 取得policyのURL/limits正本との照合、実bytesのtotal再計数が不足。
3. known cap/incomplete/context partialがtool failureへ潰れ、宣言complete=falseも分類不十分。
4. evaluatorがcontainer/query/range/checksum/検証flagへ十分結ばれず、偽reportでacquiredにできる。
5. 旧handoff・未承認後変更trialが採用資料のように混在。
6. review submitted_at、固定file fetched_at、直接object URLの不足。
7. binary/sourceの関係が自己申告のみ。実build工程と全成果物hash連鎖が必要。

snapshot safe-readとpolicy正本を共有し、reportにhit文書だけのfull bytesを保持して
recipient/evaluatorが同じvalidatorで再hash・query/hit/context照合する経路へ整理した。
旧trialは保護して採用indexから分離し、実build・pack前freeze・実行証拠を追加した。

## 指摘範囲の確認: Request changes

既指摘の解消とcanonical実行証拠のhash対応を確認したが、取得器が未使用`body_objects`を
追加しproducerの厳密record schemaで拒否される不整合が残った。
未使用field・重複timestamp処理を削除し、`acquire.RECORD_FIELDS`をproducerでも共有。
保存9 responseを再生する完全offlineの取得→producer回帰でred/greenを確認した。

## 最終判定（全文）

Approve（READ_ONLY / STATIC_ONLY）。

残っていた acquisition record schema の指摘は解消されています。

- acquire.py が `RECORD_FIELDS` を正本化
- acquire の record 出力は同じ9 fieldだけになり、未使用 `body_objects` と誤った timestamp 複製を削除
- produce.py が `acquire.RECORD_FIELDS` を直接参照
- review の `submitted_at` と固定 file の `fetched_at`／object URL は canonical raw から構成
- test_case.py は保存済み9 responseを完全 offline で再生し、取得結果を producer が直接受理する契約を確認

更新後の canonical index にある12ファイルの size／SHA-256 は実ファイルと一致しました。
run-freeze の acquire、produce、test hash も現在 bytes と一致し、questions、evaluation、
snapshot、policy は従前 hash のままです。保存結果は manifest `952855…`、report `b7fc397…`、
q1〜q6 acquired、q7 record_insufficient、検証文書7件です。

これは静的判定です。報告された Python 10件と phase61／phase60 wrapper は再実行していません。
CI、security、Companion、PR、merge は未実施・未確認です。
