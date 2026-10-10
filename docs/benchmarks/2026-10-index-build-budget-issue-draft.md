# 検索index構築へ共有資源予算を導入し、既存データを保護する

## 背景・納品Goal

公開pre.6のC2/C4 100 MiB/ngram評価は外部メモリ監視で中止した。
OOM/製品不具合や成功時peak RSS/時間は確定していない。
ローカルで完成した構築予算を、製品・回帰・契約・必要な証拠の1 PRとしてmainへ納品する。
予算内なら従来bytes/検索結果を保ち、超過前に製品自身が明示拒否する。

base: `9cf2c7c66a88c9f37a67567e5fa2247941b93dea`。
既存#289/#27/#295/#316は関連するclosedテーマ。#31/P2整理や次のreleaseへ自動継続しない。

## 変更内容と利用条件

- IndexBuildLimitsの共有既定: granules1,000,000、distinct keys262,144、posting IDs8,000,000、
  distinct key bytes16 MiB、encoded phase128 MiB。token/ngram、library、QZI builder、
  sidecar-rebuild、sidecarなしsearchへ適用する。
- named構造/phaseのlogical予算。厳密な全process RSS上限/OOM防止ではない。
- 不正CLI引数は副作用前exit2、超過/overflowはResourceLimitExceeded/exit1。
  部分index成功やraw-scan fallbackなし。入力QZTと既存QZIを保持し、新規完成品を残さない。
- **今回のC2/C4 10 MiB/ngramは新既定で拒否する。** posting16Mを明示した限定確認では
  従来QZI bytes/検索結果が一致した。既定受理範囲は縮小する。
- 100 MiBは製品の制限拒否と既存データ保護を確認。完走、成功peak RSS、性能改善は未確認。
- 公開Rust optionsへlimits追加、token from_parts引数変更、
  with_line_limit→with_options/SidecarBuildOptionsのpreview source移行。
  Reader/query制限、成功QZT/QZI形式は維持する。
- 公開pre.6には新optionがない。同じversion文字列のlocal source buildも公開binaryとは別identity。

## 証拠と受入

ローカル完了の514 PASS/3 ignored、package、条件付きMSRV、Sol medium専門reviewと
Companion ALLOWは対象bytesを再照合して再利用する。
それらは今回の追加差分や最終PR HEADの承認ではない。

原302 MB archiveは削除せずlocal-onlyで保持。Gitには小hex fixture/test、契約docs、
結果report、raw record/text output、失敗/未実施、review receipt、未同梱fileの所在/hash/reason、
固有pathに依存しないfixture生成/照合手順を含める。
未同梱binary/QZI/corpusをGitHubで第三者が原本確認できるとは説明しない。

- [x] 元の完了receiptとsource/test/docsを照合し、無関係な変更・公開artifactを保護する。
- [x] 今回の証拠整理/再現手順を対象検証し、新しいCompanion Gateを通す（ALLOW）。
- [ ] 1 PRの最終HEADに結び付いた通常CI/securityが成功し、受入に関係する指摘を解消する。
- [ ] repository規則に従いmainへmergeし、同じmerge SHAのmain CI/securityを確認する。
- [ ] PR/merge SHA/結果/制約を記録してこのIssueを完了する。

## 境界

このIssueのみbranch/commit/push/PR/通常hosted CI/review対応/main merge/Issue更新を行う。
tag/Release/crates.io/公開artifact変更、format/spill/sort/容量最適化/競合比較/P2 refactorは含めない。
G3全体・第三者検証・production-ready・大規模完走・公開版への収録は未達のまま。
