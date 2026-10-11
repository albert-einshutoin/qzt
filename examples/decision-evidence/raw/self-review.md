2026-10-11 / Issue346 / scoped self review

Core/API/search budgets/CI/署名基盤へ変更なし。実装はfixture・producer・recipient・既存方式のPython E2E/Rust launcher・資料だけ。原文由来の座標/checksum/query期待値をpack前に保存し、受取側コードには原文/回答/fixture importを含めない。

manifestの外部期待SHA→schema/container/Document Index座標→Deep/doc/range→原文由来SHA→ngramhit/contextの順で検証。unknownは固定された記録不足、必要refs欠落/破損/不正statusはError。unsigned信頼、coverage unknown、架空承認/実行許可の非同一性を明記。既存出力の再実行を先に拒否し、QZIは不在時だけ生成。

対象Python4テスト、既存phase21 3テストと新Rust launcher1テストが成功。初期RED4失敗、phase21のinternal-testing未指定によるcompile exit101も保持し、修正commandの成功と分離した。16 subprocess refusalと4不正search-statusチェック、要約更新と派生QZI再構築/原文不変を確認。専門レビューと完全gate/hostedは後続で確認する。
