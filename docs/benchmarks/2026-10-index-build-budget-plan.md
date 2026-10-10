# 検索index構築予算: 実装前計画

2026-10-10 JST。main `9cf2c7c66a88c9f37a67567e5fa2247941b93dea`。
新detached worktree `/Volumes/Satechi/Developer/qzt-index-budget-20261010`、空き約579 GiB。
元checkoutと前Goalの未コミット成果物・公開pre.6・過去manifestは読み取りのみ。
既存196 Issueの全title/stateを再取得。同一openテーマなし。関連#289はquery/行制限、
#27は構築表現改善、#295/#316は計測/open改善でclosed。#31等へ自動継続しない。

## 小規模profileと判断

変更前をRust1.96.0、`cargo build --release --locked --bin qzt`、既定featuresで固定。
前GoalのC2/C4 10 MiB QZTとhashを再利用し、ngram構築だけを各1回、30秒上限でprofile。
`sample` 1秒、`heap -s`、`MallocStackLogging=1` を使用。instrumented値は性能比較に使わない。
子peak RSSはC2 170,033,152 B、C4 243,056,640 B。両方成功、外部中止なし。
CPUは `emit_line_granule` 配下の `ngram_keys` とposting map処理を観測。
heapではdecoded chunk 262144 B、BTree node allocation、ngram key allocationを観測。
heap取得時点のall-zoneはC2 37,424,688 B / C4 61,835,008 Bで、peakや構造別総量ではない。
sourceではgranule/key/postingが全入力にわたり保持され、行key Vecとscalar境界Vecは一時、
その後encoded posting/skipとQZI section/outputが重なる。profileの正確な配分はunknown。

この根拠から、一つの `IndexBuildLimits` 契約で次の独立した保持量・符号化量を制御する。
原文サイズに推測倍率を掛けるRSS保証は作らない。

| field / CLI | 既定 | 検査位置 |
|---|---:|---|
| max_granules / --max-build-granules | 1,000,000 | 次のgranule push前 |
| max_distinct_keys / --max-build-keys | 262,144 | 新しいmap keyのcopy/insert前 |
| max_posting_ids / --max-build-postings | 8,000,000 | 新しいkey/granule pairのpush前 |
| max_key_bytes / --max-build-key-bytes | 16 MiB | 新しいdistinct keyの累積UTF-8 byte copy前 |
| max_encoded_bytes / --max-build-encoded-bytes | 128 MiB | transientのposting+skip合計、またはQZIのgranule+term+posting sections合計のencoding前 |

inclusive、0はその量を許さず、overflowもResourceLimitExceeded。重複keyは同じ行では
posting1件。同一keyが次の行に現れるとposting1件追加。key bytesはdistinct保持byteのみ。
key/posting/granuleはlogical数で、Vec spare capacity、BTree node、allocator、全process RSS
ではない。encoded budgetは各名前付きphaseの合計で、同時live全buffer合計ではない。
QZI header/manifest、decoded chunk、line carry、token正規化scratchは除外し、既存Reader/
16 MiB line制限が別途適用される。これらの除外を利用者向け契約へ明記する。

既定値は64 MiB相当のu64 posting payload、約1百万granule、辞書/key byte16 MiB、
encoded128 MiBを独立した操作予算として選ぶ。観測した150〜230 MiB級の10 MiB構築と、
前Goalの100 MiB/ngramの1 GiB監視停止の間に、製品の明示拒否を置くための初期既定である。
元の受理範囲は縮小し得る。10/100 MiB完走や改善率は受入条件にしない。

## canonical経路

RawToken/RawNgram build optionsへlimitsを追加。共有line builderへmeterを渡す。
行key列の全量生成を、元のtoken/Unicode-scalar semanticsの逐次発行へ変更し、次の保持量を
検査する前に大量keyを作らない。query tokenizer/planner/Reader上限は変更しない。
materialized `from_parts` も明示limitsでencoding admissionを確認する。
QZI読込は既存SidecarLimitsを明示Reader profileへ写像し、Readerの受理制限を縮小しない。
Reader profileをsource buildのfallbackには使わない。

sidecarは `SidecarBuildOptions { max_line_bytes, limits }` と
`build_search_sidecar_from_file_with_options` をcanonical custom入口にする。
旧 `with_line_limit` はtechnical-previewのclean breakとして削除し、呼び出し/test/docsを
同じ変更で更新する。既存のdefault memory/file convenience入口は同じcanonical入口へ委譲。
公開Rust完全struct literalには新fieldが必要。QZT/QZI形式は変更しない。

CLI search（sidecarなし）とsidecar-rebuildの5optionは共有parserで副作用前に検証。
不正引数exit2、超過/overflowexit1、部分indexを返さない。既存QZIと入力は原子出力経路で保護。
--sidecarと構築optionの併用は副作用前のusage errorにし、silent ignoreを避ける。
raw scan、旧経路、spill、外部sort、依存追加は行わない。

## 検証と安全な実行枠

先に小fixtureの回帰テストを追加。空入力、各inclusive境界と超過、counter/encoded arithmetic
overflow、不正CLI/併用、同一行重複/複数行、UTF-8/emoji/CRLF/長行、library/file/memory/
transient/sidecar適用、既存出力・入力保護、新規出力なしを確認する。
従来byteの小goldenと変更前binary生成QZIに照合し、検索結果・source bindingも確認。

計測はC2/C4の1/10 MiB（token/ngram before/after各3回、OS cache非制御）を基本に限定。
公開pre.6のidentityは前Goalから読み取り確認し、改善比較は同toolchainのsource before/afterのみ。
小fixtureで製品拒否を確認した後、C2/C4 100 MiB/ngramだけafter既定予算で各1回。
before100 MiBは前Goalで外部中止があるため再実行しない。9条件全体の再実行もしない。
各180秒、全体3600秒、生成物8 GiB、空き10 GiB。
ps監視RSS2 GiB、OS critical、またはwarning+RSS1 GiBで外部中止し、製品拒否と区別する。
成功/製品資源拒否/error/外部中止/timeout/未実施を全rawに残す。automatic retry0。
想定計測10〜30分、完全検証・reviewを含めて30〜90分、追加外部サービス費用なし。

高リスク順序: 対象検証→セルフレビュー→Rust/資源境界専門レビュー1件 Sol medium
→指摘一括修正→指摘範囲の最終review→`make check`、必要な公開API/package検証
→Companion Stop Review Gate。完全CIはreview確定前に起動しない。hosted CIは起動しない。
2巡で同根本原因が解決しなければ局所patchを止め共有設計を見直す。
納品は製品コード/test/docs、契約、限定計測、再現手順、Issue本文案、reviewと制約まで。
commit/push/Issue/PR/merge/tag/release/第三者送信は行わない。G3全体/production-ready未達。

## 検証中の追記（元の予算は変更なし）

10 MiB/ngram既定8M postingはC2/C4で製品拒否を確認した。
実行前に固定したmeasure driverの分岐に従い、posting16Mを明示した小control各1回で
before QZI bytes/検索結果を照合した。defaultの残り2反復とdefault後続searchは未実施。
100 MiBは既定で製品拒否/保護を各1回だけ確認し、before大入力や他条件へ広げなかった。

最初のmake checkでは旧10 MB成功時value benchmarkがResourceLimitExceededで失敗した。
逆依存への必要最小限の修正としてReleaseBenchmarkOptionsに明示index_build_limitsを追加し、
canonical builderへ渡す。製品既定は8Mのままで、そのtestだけ16Mを明示する。
supplied予算の伝播を小regressionで確認し、同じ専門review担当によるこの差分の最終確認後に
make check/package/MSRVを再実行する。失敗logや初期計測は上書きしない。
