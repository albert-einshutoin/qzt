# セルフレビュー

Core/公開API/CI設定/依存の変更なし。公開8 URLを取得前allowlistに固定、9本文41093 bytesをraw response/json pointer/object ID/固定commitと照合。引用byteと意味判断は分離。回答者に正解/offset対応を渡さず、Cは自分のsearch hitからcontext→doc。B/Cの出典・要約・質問は共通。gateway childのみnetwork/write拒否、Agent全体のOS隔離は主張しない。

known cap/incompleteはpartial_unknown、通常もcoverage_unknown。破損・不正参照・未知reasonはError、QZI破損時に再構築しない。原文が欠けた場合は推測せずmissing。無提出はactor unknownと区別する。対象8テストと#348校正10テスト成功。回答開始後の質問/採点/製品変更は禁止。

既存文書の不整合: docs/CLI.ja.md冒頭はcontextを含む公開pre.6と説明するがcontext節は開発版source専用と記す。公開source 0c8110e6 のsrc/main.rsはcontextをdispatchする。今回その文書は変更せず、実験binaryをmain source4681072とbinary SHAで固定し公開binaryと同一扱いしない。
