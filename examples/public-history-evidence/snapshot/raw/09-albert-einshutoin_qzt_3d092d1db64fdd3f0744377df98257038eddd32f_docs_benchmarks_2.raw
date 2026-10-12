# 構築資源予算のGitHub納品

2026-10-11。実装baseと開始時remote mainは
`9cf2c7c66a88c9f37a67567e5fa2247941b93dea`。
この資料は過去のローカルGoalと今回のIssue/PR/main納品を分ける。
製品の5 source file、回帰test/fixture、契約docsは過去の完了manifestへ再照合してから取り込んだ。
既定値・設計を再調整せず、CI基盤・公開pre.6 tag/assetは変更しない。

## 差分と証拠の扱い

製品、必要test/小hex vector、契約/CHANGELOG、過去結果report、コンパクトな証拠と再現手順を同梱。
原archiveは約302 MBで、原本は実装worktreeに保持する。binary、生成QZI/corpus、重複driver/
snapshotはGitへ追加しない。
[納品証拠index](2026-10-index-build-budget-delivery-evidence.json)に、同梱原本、embedded stdout/stderr、
未同梱fileの元の所在・SHA-256・size・理由を列挙する。hashやpathだけでは未同梱fileを
GitHubから取得・確認できない。ローカル限定の原binary/生成QZIを第三者確認済みと説明しない。

過去の[結果report](2026-10-index-build-budget.md)、[計画](2026-10-index-build-budget-plan.md)、
`completion-receipt.json`、専門review、Companion ALLOWは当時の記録である。
「commit/外部納品未実施」はその時点の状態であり、今回のPR/main状態ではない。
過去の数値、失敗、review verdictを上書きしない。
新しい納品整理のGate、最終PR HEAD CI/security、merge SHA main CI/securityは今回の別証拠になる。

`measurement-initial/records.jsonl` と `final-binary-readback/records.jsonl` は元のbyteを同梱。
それらの絶対pathは計測host上の歴史的pathで、checkout内のfileを意味しない。
stdout/stderr全文は `raw/2026-10-index-build-budget/attempt-output.jsonl` のphase/labelで取得でき、
各streamのoriginal hashで元byteに対応する。122試行の成功/製品拒否をすべて残している。
旧make check失敗、MSRV host SDK失敗、未実施理由も同梱し、成功だけを抜粋しない。

## 利用者へ明示する変更

- 共通予算はgranules1,000,000、distinct keys262,144、posting IDs8,000,000、
  distinct key16 MiB、encoded phase128 MiB。library/QZI/sidecar-rebuild/transient searchへ適用。
- 今回のC2/C4 10 MiB/ngramは新既定で拒否する。posting16Mを明示した限定確認では
  従来QZI bytes・検索結果が一致した。既定の受理範囲は縮小する。
- 100 MiBは製品自身の制限拒否と入力/既存出力保護を確認した。完走、成功peak RSS、
  性能改善を確認したとは扱わない。
- build optionsへのlimits追加、token from_parts引数変更、with_line_limit→with_optionsの
  preview source移行を[API方針](../API_STABILITY.md)へ記録。
- 現在公開中のpre.6は新optionを持たない。version文字列が同じローカルbuildでも
  binary/source hashが異なり、公開binaryの確認と混同しない。

これは名前付き構造/phaseの予算で、厳密なRSS上限/OOM防止ではない。
G3全体、第三者検証、production-ready、大規模完走、公開版への収録は未達。

## 今回の納品計画

1. 原manifestと製品/test/docsを照合し、原本を保護した別worktreeでこの差分だけ整理。
2. 固有pathに依存しないfixture生成/照合と再現手順を対象チェックする。
3. 今回の追加差分/証拠整理をセルフレビューと新しいCompanion Gateで確認。
4. 1 Issue/1 PRの最終HEADで通常CI/securityを確認し、規則に従いmainへmerge。
5. 同じmerge SHAのmain CI/securityを確認してIssueを完了する。

開始時に同テーマのopen Issue/PRはなく、mainのbranch protection/rulesetsは設定なし。
管理者bypassやcheck無効化は使用せず、通常CI/securityと未解決reviewを確認する。
製品bytesが変わらなければ、過去の514 PASS/3 ignored、package、条件付きMSRVと
専門reviewを対象hash付きで再利用し、profile/大規模計測/完全localtestを重ねない。
過去ALLOWを新しい差分・PR HEADの承認には使わない。

Issue/PR/HEAD/merge SHAと今回のCI結果は、納品進行に応じてこのGoalの記録へ追加する。

## 納品整理の最終確認

Issue: [#344](https://github.com/albert-einshutoin/qzt/issues/344)、branch:
`codex/index-build-resource-budgets`。
初回納品整理のfresh Companion Gate（gpt-5.6-luna / medium）は **ALLOW**、touchedFiles=[]。
receipt/logは`delivery-companion-gate.json`と`delivery-portable-check.log`。
その時点の製品5 source hash一致、44同梱原本、122 records/244 embedded streams、95未同梱の
境界、portable fixture照合、過去と新納品状態の区別を確認した。
この判定はPR HEAD CI/securityやmerge後main検証の成功を含まない。
その結果はGitHubの同一HEAD/merge SHAのrunとIssue完了commentへ記録する。

## PR初回検証への対応

PR: [#345](https://github.com/albert-einshutoin/qzt/pull/345)。
初回HEAD `31457cb94b3a234970d7c5d0b596b6e2b33967e7` の
[CI](https://github.com/albert-einshutoin/qzt/actions/runs/38065365996)と
[security](https://github.com/albert-einshutoin/qzt/actions/runs/38065366327)に失敗があった。
失敗ログは`delivery-fix-*-first.log`へ保持した。

- Rust1.99のClippy追加lintに合わせ、空stdoutの同じ期待値を`assert_eq!`へ変更。
- 生成adapterのargv依存を除き、4096 bytes/4行のstdin契約へ変更。
  portable再現と不正入力・既存出力保護の5チェックを通した。
- secret検出の対象は小hex fixtureへ照合したSHA-256値だった。元のJSONをlocal-onlyの原本として
  所在/hash付きで保持し、同じ値を型付きdigest inventoryへ派生した。検出器の除外設定は追加しない。
  現在の同梱原本は43、未同梱96、embedded streams244。元の44/95の初回receiptは過去状態として保持する。
- PRのP2指摘に対し、既存profile matrixへ5つの明示予算を渡す入口を設けた。
  製品defaultの資源拒否を非成功として記録し、後続caseへ進む。その他のerrorは失敗させる。
  自動増額はせず、100 MBのmatrix完走を今回の成果に含めない。

修正後の対象検証はphase49 10 PASS、release_hardening 4 PASS / 2 ignored、Clippy成功、
C2/C4 1 MiB再現成功、生成adapter拒否5件、local gitleaks findings 0。
Sol mediumの範囲限定専門reviewはApprove。製品5 source bytesは元の完了receiptと一致し、
製品の再設計・大規模計測・全local test・CI基盤変更は行っていない。
今回のfix logsと`delivery-fix-specialist-review.md`を同梱する。

初回commitにはsecret誤検出の旧JSONが含まれるため、自分の単一commitをamendし、
remote HEAD照合付きのforce-with-leaseで同じPRを更新する。初回SHA/失敗証拠は上記に残す。
最終HEADの通常CI/securityとmerge SHAのmain検証は別途確認する。

修正差分のfresh Companion Stop Review Gate（gpt-5.6-luna / medium）は **ALLOW**、
touchedFiles=[]。`delivery-fix-companion-gate.json`に記録し、
初回Gateや最終hosted検証と分ける。原本/派生の最終hash照合も成功した。
