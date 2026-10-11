# 固定利用例の確認結果

2026-10-11 / Issue #346。base product source: 3d092d1db64fdd3f0744377df98257038eddd32f。
公開pre.6ではなく、このmain sourceのローカルbuildを使った。
binary versionはqzt 0.1.0-pre.6だが、公開版同一ではない。
実行物SHA-256: 1a9406931a21ead8bf0475c24be43c71d6f6e6fc484e3eff0f0a6e9fd70ec9f1。

初期handoffはQZT、要約、manifest、qzt、recipient.pyの5 file。原文/oracleは渡していない。
recipientはcwdの固定名だけを読み、元repoの原文pathを参照しない。OS sandboxや独立第三者実行の証明ではない。

| 問い | 根拠doc_id | 原文全体の半開byte範囲 | 結果 |
| --- | --- | --- | --- |
| q1 | decision/01-proposal.txt | 0:454 | 原文byte/期待checksum一致 |
| q2 | decision/02-hold.txt | 454:687 | 原文byte/期待checksum一致 |
| q3 | decision/04-conditional.txt | 950:1327 | 原文byte/期待checksum一致 |
| q4 | decision/03-objection.txt | 687:950 | 原文byte/期待checksum一致 |
| q5 | decision/05-change.txt<br>decision/06-revision.txt | 1327:1610<br>1610:1962 | 原文byte/期待checksum一致 |
| q6 | decision/07-latest.txt | 1962:2288 | 原文byte/期待checksum一致 |
| q7 | 根拠未提示 | なし | unknown |
| q8 | 根拠未提示 | なし | unknown |

回復した具体的な条件・反対・変更理由はREADME.ja.mdの固定表と、raw/recipient-report.jsonの各evidence原文を参照する。
要約の「AからBへ変更」だけでは分からない、一日100件以下/短時間通信断/翌日確認、
書き直しへの反対、一日300件・72時間断という計画変更、A撤回/B条件付き限定試行を、指定docから確認した。
q7の実際の全員合意とq8の未記録の応答時間はunknown。計画値を実測値と扱わない。

q5は原文から固定した「前提変更：」のngram hitをcontextへ渡し、doc_id・local/global範囲とhex bytesを照合した。
search coverageはfalse/unknown。選んだfixture/queryの一致で、一般網羅性や不存在を証明しない。

要約更新のexit0、QZT/manifest/旧report/log不変、QZI再構築後同一bytes、同じ出力先への再実行exit1と既存bytes保持を確認した。
raw/summary-update-and-retry.jsonにcommand/終了結果とhashを残した。

実装前RED4失敗と、既存phase21のinternal-testing未指定によるcompile exit101を保持。
feature付きの修正commandは既存phase21 3PASS＋新Rust launcher1PASS（内部Python4PASS）。
raw/rust-targeted.log、python-targeted-final.log、test-attempts-final.jsonl等に区別して残した。
checksum期待値と復元原文との照合漏れを防ぐ追加回帰は別ログで記録した。
現在の24 subprocessは7成功/17拒否。manifest内のdoc/ref両方を同じ誤checksumへ変えても、
復元byteとの外部SHA照合で拒否する。raw/validation.jsonにcommand/cwd/base/exit/log hashを記録し、初回結果を消さない。

専門review/Companion/完全local gate/最終PR HEAD CI/security/merge SHA main検証は、完了後の別証拠として記録する。

これは固定された架空記録での情報回復。unsigned manifestは信頼済み経路の期待hashを前提とし、
本人性・真実性・時刻・実際の合意・現在の実行許可を証明しない。LLM精度向上、独立第三者検証、production-readyは未達。
Core/API/予算/CI/署名基盤・公開pre.6 tag/assetを変更せず、次フェーズ/releaseへ継続しない。

## ローカル最終検証

Sol medium専門レビューはApprove、provenance指摘は直接範囲の再読で解消。
Companion fresh GateはALLOW、touchedFiles=[]。make checkはexit0、各test summaryの
合計516 PASS/0 FAIL/3 ignored。package --allow-dirty --lockedもexit0。
profile/大規模資源benchmarkは再実行していない。gitleaksの追加file scanはfindings 0。
完全gateの例外的なignoredを成功へ置き換えない。

execution.jsonはpositive replay時点の記録。そこで記録したtest_case.pyのhashは追加回帰前の
歴史的helper identityで、現行helperと同じbytesとは扱わない。producer/recipient/fixtureの
identityは変わらず、現行のtest helperはvalidation.jsonとdelivery-manifest.jsonへ別に固定する。
CLI実行物はdebug/internal-testingのtargeted Cargo実行後のものをhandoffへcopyして保持した。
公開pre.6 binaryとは区別し、再実行したbuildのhashはその都度新しく記録する。
