# 検索index構築の明示的資源予算

2026-10-10〜11 JST、ローカル納品。**共有構築予算を製品へ導入し、超過前の拒否と
入力・既存出力保護を確認した。** 予算内の小fixtureでは変更前のQZI bytes、Unicode-scalar
ngram、posting順序/重複排除、source binding、検索結果を維持した。
C2/C4の100 MiBは新既定で製品による制限拒否となり、完走や性能改善とは扱わない。
公開pre.6、前Goal成果物、元checkoutの未コミット変更は保護した。

## 実装した契約

`IndexBuildLimits` はtoken/ngramが共有するinclusiveな5予算。

| field / CLI | 既定 | 拒否する前の処理 |
|---|---:|---|
| max_granules / --max-build-granules | 1,000,000 | 次の行granule push |
| max_distinct_keys / --max-build-keys | 262,144 | 次のdistinct dictionary keyのcopy/insert |
| max_posting_ids / --max-build-postings | 8,000,000 | 次のdistinct key/行pairのpush |
| max_key_bytes / --max-build-key-bytes | 16 MiB | distinct key保持byteの増加 |
| max_encoded_bytes / --max-build-encoded-bytes | 128 MiB | transient posting+skip、またはQZI data sectionsのencoding |

同一行で同じkeyを繰り返してもpostingは1件。次の行では新たに1件課金する。
0はその単位を許さず、超過とchecked arithmeticのoverflowは`ResourceLimitExceeded`。
CLIは不正引数exit2、実行時超過exit1、成功report/部分indexを返さない。
sidecar-rebuildは構築成功後だけ既存atomic outputへ入るため、入力QZT、既存QZIを保持し、
新規完成品を残さない。raw scanや旧経路への暗黙fallbackはない。

適用入口はRawToken/RawNgramの`build_from_container`/`build_from_file`/`from_parts`、
QZI default memory/file/custom、CLI sidecar-rebuildとsidecarなしsearch。
既存のquery/Reader制限は変えていない。QZI reconstructionは既存`SidecarLimits`を明示的に
共通constructorへ渡し、source-build既定をReaderへ持ち込まない。
`--sidecar`付きsearchと新build flagsの併用は、構築がないため副作用前にexit2。

Raw build optionsへ`limits`を追加、token `from_parts`へ明示limits引数を追加した。
custom QZI入口を`build_search_sidecar_from_file_with_options` / `SidecarBuildOptions`へ
統一し、旧`with_line_limit`を削除した。公開Rust完全literalの移行とpreview clean breakは
[API方針](../API_STABILITY.md)とCHANGELOGへ記録。format/依存/並行処理は変更していない。

予算はlogicalな構造数/key長と各encoded phaseのbyte合計。Vec spare capacity、BTree node、
allocator、Reader decoded chunk、line carry、1つのtoken正規化scratch、QZI header/manifest、
callerが既にmaterializeした配列や構築後のpublic mutationを含めた全RSSを保証しない。
encoded sectionsと最終output/raw structuresは同時に存在し得る。128 MiBを全live bufferの
合算上限として説明しない。独立したReader/16 MiB line制限と外部監視が別途必要である。

## 既定値の根拠と受理範囲縮小

[実装前計画](2026-10-index-build-budget-plan.md)はC2/C4 10 MiBだけの短いbounded profileで
固定した。1秒sampleとheap、MallocStackLoggingを使い、行keyの一時生成、posting map、
decoded chunk/BTree allocationを観測。instrumented子peakはC2 170,033,152 B、
C4 243,056,640 B。heap取得時のall-zoneはpeakでも構造別配分でもない。
sourceのgranule/key/posting保持と、後段のencoding一時bufferを合わせて予算単位を選んだ。
元のngram scalar境界/key Vecを逐次発行へ変更し、保持量検査前に行全体のkey列を作らない。

初期既定はposting payload64,000,000 B（約61.04 MiB）、key bytes16 MiB、
granules1百万、encoded phase128 MiBを別々に管理する保守的な操作予算。
sourceサイズの推測倍率ではなく、全RSS/OOM防止保証でもない。
**C2/C4 10 MiB/ngramは8M postingの既定で拒否された。**
従来受理した入力を新既定で拒否する縮小を隠さない。
同じ10 MiBをposting16Mへ明示増額した小controlでは成功bytes/検索結果が変更前と一致した。
100 MiBで上限を引き上げる実験は行っていない。

## Source・binary・入力の結合

base/latest mainは `9cf2c7c66a88c9f37a67567e5fa2247941b93dea`。
新detached worktree `/Volumes/Satechi/Developer/qzt-index-budget-20261010`、空き約579 GiB。
公開pre.6の製品source `0c8110e6b6e08513b4def3f636b5b79d068e275f` とは区別する。

| binary | SHA-256 | 扱い |
|---|---|---|
| public pre.6 | `00fa5541827b7a36199fecc9e5fd967900ca1f06b7866b38cea22c53e3721bd2` | 前Goal固定公開物のidentity照合のみ。改善比較に使わない |
| source before | `c4b1eebd2dae7f4eb7c9fe80ab538c6ad2d15cb846e635f05ec3770dabbc3263` | main9cf2c7c、Rust1.96.0、release/既定features |
| initial source after | `7c72a9138f72c7be2cdd733bdd2086925304d92227b7afd11d0e56a56d5b48e5` | 共有builderの4 source hashに結合した初期計測 |
| final source after | `0f3c589ffa081141b07d3419bbf8e856329eab002ac10b2ecc7fee8dab0a1304` | benchmark逆依存修正後、最終納品/26試行readback |

source before/afterは同じ`cargo build --release --locked --bin qzt`、Rust1.96.0、追加flagsなし。
version文字列は各source buildでも`qzt 0.1.0-pre.6`だが、公開artifactとは別のsource/binary。
初期afterの4実装fileはfinalとbyte同一。追加されたbenchmark入口でfinal binary hashが
変わったため、初期統計をfinal binaryの性能値へ読み替えず、別readbackを取得した。
finalの5 source hashesは`final-binary-readback/manifest.json`。

入力は前GoalのC2/C4 seed295、1/10/100 MiB、実サイズはmarker込み。
1 MiB=1,048,618 B、10 MiB=10,485,802 B、100 MiB=104,857,642 B。
使用QZT/元corpusのhashとseedは`fixture-bindings.json`、必要なC2/C4 fixtureだけを参照。
前Goalの大量rawを一括コピーせず、指定文書・plan・summary・code/public binaryの
8対象について以前のdelivery manifestとbyte identityが同一と確認した。

## 限定計測と正しさ

macOS27.0 (26A428)、Apple M4、32 GiB、Satechi APFS、同時実行1、OS cache非制御。
各command180秒、全体3600秒、計測生成物8 GiB、空き最低10 GiB。
critical pressure、RSS2 GiB、warning+RSS1 GiBで外部中止するbest effort monitorを
前Goalから再利用した。samplingは厳密なOS上限ではない。
wallはtime wrapperのlaunch〜blocking wait、RSSはmacOS timeの子peak **byte**。
monitorはKiB、exit_codeはtime wrapperのcode。

初期測定96試行は92成功と4製品資源拒否、外部中止/timeout0。
warmup0、通常成功buildは3反復、拒否は最初の1反復で残り2回を未実施にした。
拒否したdefault後続searchも未実施。posting16Mの明示controlは各1試行で別条件。
before100 MiBと過去9条件全体は再実行しなかった。
各試行・失敗・未実施理由はrawに残し、outlierを削除しない。

初期source buildの比較表は`wall ms / peak RSS MiB`の中央値（成功n=3）。
拒否と増額controlはn=1で、中央値による性能改善の比較対象ではない。

| corpus / index | source before | initial source after（既定） |
|---|---:|---:|
| C2 1 MiB token | 36.91 / 8.47 | 37.61 / 8.55 |
| C2 1 MiB ngram | 139.65 / 21.23 | 122.52 / 22.02 |
| C4 1 MiB token | 47.58 / 14.36 | 45.47 / 14.45 |
| C4 1 MiB ngram | 129.37 / 23.16 | 112.44 / 23.70 |
| C2 10 MiB token | 180.18 / 52.75 | 156.94 / 51.97 |
| C4 10 MiB token | 275.85 / 76.06 | 239.81 / 76.73 |
| C2 10 MiB ngram | 1214.02 / 149.31 | 製品拒否677.91 / 105.88（n=1） |
| C4 10 MiB ngram | 1141.70 / 190.84 | 製品拒否626.09 / 103.16（n=1） |

10 MiBの増額controlはC2 1047.78 ms / 150.50 MiB、C4 955.87 ms / 193.98 MiB（各n=1）。
全成功条件のQZI bytesはbeforeと一致。rare/missing/common-capped検索のreportは
query_time_msを除きbefore/afterで一致し、全返却hitを原文byteへ照合した。
coverage unknown、capは維持され、一般網羅性へ広げない。

final binaryのreadbackは26試行、24成功＋2製品資源拒否。
1 MiB token/ngramのC2/C4はbefore QZI bytesと一致。
10 MiB増額ngramもbefore bytesに一致し、計18 queryの原文hitを照合した。
実測n=1のC2 1 MiB tokenは279.30 msだった。この値もrawへ保持し、初期3標本の
中央値と混ぜず、再送/除外で都合のよい数値へ置き換えていない。

finalの100 MiB/ngram既定は次のとおり。既存出力は成功した有効QZIを置き、
拒否後にその全bytesと入力QZT hashが不変であることを確認した。

| corpus | status / exit | wall ms（n=1） | peak RSS byte / MiB |
|---|---|---:|---:|
| C2 100 MiB | 製品資源拒否 / 1 | 675.94 | 111,362,048 / 106.203 |
| C4 100 MiB | 製品資源拒否 / 1 | 574.72 | 107,839,488 / 102.844 |

これらはstdoutなし・正確な既存resource diagnostic・出力/入力保護を伴う通常の拒否。
外部SIGKILL/timeoutではない。拒否の時間/RSSは完成時の値や改善率ではない。
前Goalの外部中止した100 MiB構築の成功値はunknownのままである。
finalの全source/binary identityと各command/設定はraw manifest/recordへ結合している。

## 検証・レビューと失敗の処理

最小CLI回帰は修正前にexit2対期待exit1で失敗。新規10テストでは空入力、各予算の
inclusive境界/超過、繰返しkey/複数行、emoji/CRLF/長行、library/materialized/CLI適用、
encoded skipとQZI sectionの境界、入力/有効既存出力保護、新規完成品不在を確認した。
private counter overflow1テストは増加前の拒否を確認。
beforeで作った小UTF-8/CRLF/chunk跨ぎvectorのsourceとQZI bytesを固定し回帰で一致した。

対象token/ngram/sidecar/query予算・safe file output、セルフレビュー、Sol mediumのRust/
資源境界専門レビュー1担当を実施。最初の専門reviewは修正必須なし。
最初のmake checkは旧10 MB value benchmarkの成功前提で失敗し、logを保持。
製品の既定は緩めず、benchmark optionsへ明示予算を渡す最小修正を追加し、10 MB
value testだけ16Mを宣言した。supplied budget伝播の小回帰も追加。
同担当がこの差分を最終確認し、意味上の問題なし。書式失敗の推測指摘は実コマンドで
再現せず撤回され、追加行だけ整形して未修正BLOCK要因なしとなった。

最終 `make check` は **514 PASS / 0 FAIL / 3 ignored**。
fmt、全target/all-feature Clippy、default lib/bin check、全target/all-feature test、
warnings-as-errorsのall-feature rustdocが成功。
`cargo package --allow-dirty --locked` はlocal package検証成功。
MSRV1.87は既定SDK27でarm64e.x1/TAPI linker error、明示clangでもlink errorがあり、
その2失敗を保持した。既存SDK15.2をそのコマンドだけに指定してdefault lib/bin check成功。
MSRV runtime testや他host検証とは扱わない。全体toolchain/設定は変更していない。

Companion Stop Review Gate（gpt-5.6-luna / medium）は **ALLOW**、touchedFiles=[]。
受入・回帰・resource安全性・既存データ保護・provenanceの未修正BLOCK要因なし。
promptとverdict全文は`companion-prompt.txt` / `companion-gate.json`に保持した。
製品コードの実行/import/testを行わないstatic Gateであり、第三者検証へ広げない。
Gate後は結果記録と納品manifest/receiptだけを追加し、製品code・計測値を変更していない。

## 納品・残る制約

[再実行手順](2026-10-index-build-budget-reproduce.md)、
[Issue本文案](2026-10-index-build-budget-issue-draft.md)、
[raw root](raw/2026-10-index-build-budget/)を納品する。
初期/最終source/binary、profile、計測code snapshots、golden provenance、各試行、
集計、未実施理由、全検証logと専門reviewが追跡できる。
`evidence-audit.json`はGate前のcheckpoint、`delivery-manifest.json`はローカル納品の
各file size/SHA-256、`completion-receipt.json`は残っていたGate/manifest作成の完了を結合する。

未検証は構築予算を上げた100 MiB完走、一般/実データ分布、正確な全RSS上限、OOM防止、
OS-cache-cold、並行負荷、他OS/CPU/storage、GB/TB、全てのcaller予算組合せ、
独立第三者検証、production/SLA。p99や一般改善率は主張しない。
QZI容量最適化、spill/外部sort、format変更、依存追加、全repo refactorは行っていない。
今回の安全な拒否はG3全体/production-ready/大規模入力対応の達成ではない。
commit/push/GitHub Issue/PR/merge/hosted CI/tag/release/crates.io/第三者送信は未実施。
このテーマをローカルで完了し、次Issueへ自動継続しない。
