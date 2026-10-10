# Rust/資源境界の専門レビュー

担当: `/root/rust_resource_review`、role=`rust-reviewer`、`gpt-5.6-sol` / medium。
公開APIと資源境界の高リスク変更のため指定モデルを使用した。
対象: 今回のdiffと直接呼び出し元、契約docs、固定された対象検証と計測証拠。
READ_ONLY。実行・編集・network・他agentへの委譲なし。
レビュー時点の4 source SHAは `measurement-initial/manifest-final.json` と一致。

レビュー結論（担当からの返答）:

> 修正必須指摘はありません。Approve相当です。
>
> 確認した範囲では、各資源予算は増加・copy・insert・push・encode前にchecked arithmeticで判定され、token/ngram、memory/file/from_parts、sidecar default/custom、CLI transient searchに共有されています。QZI Reader再構築も既存SidecarLimitsへ明示的に写像され、source-build既定による受理範囲の縮小はありません。資源拒否時のQZT・既存QZI保護、CLI終了コード、旧API削除も契約どおりです。
>
> 提示証拠では対象テスト、overflow、golden 10件、Clippyが成功し、測定は92成功＋4製品資源拒否です。現行4 source SHAもmanifest-final.jsonと一致しました。
>
> これは指定差分と直接呼び出し元に限った読み取りレビューです。指示どおりテスト・CI・network・編集は実行していません。最終make check/package、hosted CI、merge-conflict解消、G3・第三者検証・production-ready判定は未達です。

修正必須指摘0件のため修正・指摘範囲の再レビューは不要。これが専門レビューの最終判定。
全体の最終検証はレビュー後に実施し、別logへ記録する。
このレビューはmergeや第三者検証の実施を求めるものではなく、ローカル納品に限定する。

## 完全検証で見つかった逆依存の修正確認

最初のmake checkは旧10 MB value benchmarkの構築成功前提で失敗した。
製品予算を緩めず、ReleaseBenchmarkOptions.index_build_limits（defaultは製品default）を
追加し、10 MB value testだけposting16Mを明示した。追加diffは同じ担当が読み取り確認した。
targeted release3成功/2ignored、budget10成功、Clippy成功。
token/ngram双方へ同じ明示予算が伝播し、製品/Reader既定は変わらず、意味上の指摘なし。

担当は追加行のfmt失敗を予測したが、実際のcargo fmt checkは修正前後とも成功した。
追加行だけ折り返した後、担当は推測を撤回し、次の最終結果を返した:

> 未修正のBLOCK要因はありません。該当箇所の折り返しと、空出力・exit 0のfmt-final.logを確認しました。
> 先のfmt失敗断定は未確認推測だったため撤回します。意味上のApproveは維持します。

この確認はCI失敗修正と書式指摘の範囲だけで、新たな探索レビューではない。
専門reviewの担当agent/modelは1件のまま。修正後の完全検証とCompanion Gateは別証拠。
