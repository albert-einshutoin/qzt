# 意思決定の原文参照を使う固定利用例

Issue #346。base main: 3d092d1db64fdd3f0744377df98257038eddd32f。
7件・8問・要約2版とoracleを実装前に固定した。全て架空。
6問は指定文書の原文を確認し、全員の合意と未記録の応答時間はunknown。

既存pack-docsによるDocument Index、docの検証読取、range、info、Deep verify、
ngram QZI、search、contextを再利用する。外部SHA-256期待値はpack前の原文から作る。
QZTは復元・byte検証のみ担当し、質問・意味解釈はこの固定例の外部データに置く。
Core/API/予算/CI/署名基盤の変更、LLM、実データ・他プロダクト接続はしない。

受取側は新directoryのQZT、summary、manifest、qzt、recipient.pyだけを読む。
manifestのhashを信頼済み経路で渡す前提で、unsigned manifestから本人性・真実性・
実際の合意・時刻や現在の実行許可を証明しない。原文・oracleをコードに埋め込まない。
CLIと復元原文の独立SHA-256を両方確認する。範囲は0始まりbyteの半開区間。

先に最小E2Eと拒否チェックを作り、未実装時の失敗を記録する。
container/doc/range/checksum/破損/必要根拠欠落/未知statusをErrorとし、
固定ケースの記録不足だけunknown。cap/coverage unknown/0件を不存在証明にしない。
要約変更、QZI削除後再構築、既存出力への再実行拒否とbyte保持も確認する。

各CLIは30秒timeout、小fixture合計64KiB以下、単一実行。
想定はlocal対象/E2E・完全gate・packageで数分、hosted CI各数分。
新しい外部API利用料なし。大規模profile/資源benchmarkは再実行しない。
対象検証→self review→Sol medium専門レビュー1件→修正再検証→範囲最終review
→Companion Gate→必要な完全local gate→1 PR最終CI/security→通常merge
→同一merge SHA main CI/security→Issue完了の順で納品する。
