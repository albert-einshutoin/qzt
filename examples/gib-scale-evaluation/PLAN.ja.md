# 1 GiB以上の実文書集合評価・事前計画

前回PR350/41,093 bytesの判断を上書きせず、新しい規模・コーパスの評価とする。
Core/API/format/default budgetは変更しない。回答8体B1〜B4/C1〜C4、後から匿名独立採点4体。
全てfresh context、同じmodel/effortを回答前に固定。人間・第三者評価/需要の証明にしない。

候補は2つに限定: (1) 英語Wikibooks 20261001（articles archive194.2 MB、抽出本文1 GiBは未確認）、
(2) 英語Wikipedia 20261001（分割archiveの一次配布あり）。十分な実本文を取得できる後者を採用。
技術専用コーパスとは呼ばず、百科資料の根拠調査を測る。旧9文書や合成文を加えない。
ライセンスCC BY-SA 4.0。記事ID/revision、帰属・履歴リンク、変更した抽出規則を記録。
公開原本はWikimediaの固定日付URL。将来の配布継続は保証せずlocal-only原本を保護する。

corpus.pyのURLSを取得前allowlistへ保存。最初の2つのmultistream archiveと同日sha1sumsだけ。
redirectなし、認証なし、通信合計1 GiB、同時GET1、archive毎900秒、全体1800秒。
取得のsha1を一次配布checksumへ照合し、SHA256も記録する。別hostへ暗黙fallbackしない。
namespace0・非redirect・wikitextの非空本文をpage ID順で抽出。exact本文SHAで重複除去、最初を採用。
単一本文4 MiB超は除外し件数を記録。本文末尾LFが無い場合だけLF追加を明示。
XML/metadata/archive bytesを本文に数えず、元本文bytesが1.1 GiB以上になった最初の文書境界で停止。
最大1.2 GiB。原本、文書数、言語、長さ分布、重複数、原文/正規化bytes/hashを保存する。

外付け /Volumes/Satechi/PersonalAgentArtifacts/qzt-gib-20261026 に新規保存。旧raw/receiptへ書かない。
Gitへ大きな原文・QZT/QZI/binary/全文tool traceを追加しない。manifest/hash、再取得手順、集計、小fixtureを納品。
開始時空き約395 GiB、物理32 GiB、memory pressure level1。構築は逐次1件。
子RSS4 GiB、生成物合計30 GiB、空き30 GiB未満、pressure critical4またはwarning2が10秒継続で停止。
監視はsamplingでありOSの厳密なRSS/OOM保証ではない。構築単独900秒、全構築1800秒を上限とする。

校正は同じ実本文の10/100 MiB subsetで行う。本評価に代用しない。
単一containerはcore streaming packを使い、文書ID/offsetはB/C共通の外部inventoryへ保存する。
pack-docsの全入力memory/OS argv制限を隠さず、Document Index無しという運用条件を明示する。
既定indexの拒否を一度だけ保存。分割条件は文書を跨がない連続page ID順・固定本文上限を校正後に凍結。
index種別、必要なら明示予算の追加1条件、分割上限、質問/参照列/停止条件/実行順を本評価前に固定。
全container検索なら全fan-out/CLI起動/index open/結合を計測。正解routing/cache/service/DBは作らない。

B/Cは同じ概要・質問・inventory・同じ参照情報を持ち、検索/ID直接参照/範囲読取を対称に許す。
出力は同じbounded text/citation補助。native rawはlocal-onlyで残し変換費用も含める。
各操作のbinary/QZT/QZI全量hashを製品時間へ混ぜない。開始終了の入力照合とnative検証を別に計測。
warm/unknown cache、逐次機械計測。Agent並列wallを製品速度と呼ばない。

I: 新しい自然な8〜12問（複数/離れた文書、長文中間、言い換え、unknown）を原文へ機械照合し事前固定。
II: B/C共通doc/範囲の既知参照。1/10/100回、同一点/分散の事前列を別々に計測。
初回pack/verify/index、初回照会、反復検索/部分取得、原文/QZT/QZI/metadata/補助生成物を分ける。
未知のI/O/課金はunknown。測っていない値を比例推計しない。モデルの損益分岐と実測を分ける。

unknownの正しい回答不能と未調査/無提出を先に区別。全回答固定後、同じ匿名入力をJ1〜J4へ渡す。
引用実在/範囲/SHAは別の機械確認。多数決で真実を決めず解消不能はunknown。
主規模でCが成立しない場合、準備失敗と未実施を正直に記録し、規模阻害要因の確定を成果とする。

対象検証→セルフ/専門レビュー1担当→直接指摘修正/最終確認→本条件freeze→本評価。
最後に必要な検証、Companion、最大1 PRの最終HEAD CI/security、通常mergeと同merge SHA main確認。
新Issue・release/tag/deploy・次の実装/Goal・他chat報告は行わない。

校正確定: raw本文1,181,143,804 bytes、LF正規化後1,181,167,547 bytes、23,743記事。
10/100 MiBのwhole-document subset pack/Deep verify成功、10 MiBと離れた8 MiB×3のtoken index既定成功。
本条件はcore/既定4 MiB chunk、Document Indexなし＋共通外部inventory、単一QZT既定token index1回と、
文書を跨がない8 MiB上限/page ID順142 QZT＋既定token QZI。明示予算条件は0。
全体検索は全142件を逐次fan-out。Bはrg literal whole-wordの全資料検索＋表示用rg実一致offset取得。
追加CLI/結合/正規化の費用を含み、helperの費用を製品単体へ帰属させない。
両条件はfind/read/withinを対称利用可能。withinは選んだ文書を復元し共通Python literal検索する薄い補助。
回答はgpt-6.1-sol/medium/fresh none、seed20261027、900秒/60操作/返却1 MiB、単一操作120秒。
採点は同設定fresh4体、900秒。q1-q9のunknownのみは完遂0、q10の調査済みで適切な回答不能は完遂2。
not_examined/missing/invalidは調査0・完遂0で別記録する。未提出の匿名failureは固定分類だけ。
Iの機械query5語×3回、IIの512-byte既知参照same/distributed各1/10/100を先に固定。
収集第一試行はhost pressure warning継続で停止（子family RSS約38 MiB）。旧partial/ログを保持し、
正常回復後、一次checksumで再検証したarchive1だけを再利用して一度のretryで完了した。
通信budgetには前試行分と未flush最大1 MiBの保守上限を繰越し、合計1 GiB以内。
構築監視のartifact上限はmaterial/shards/shard-inputsの実statに、収集・失敗原本・校正用4 GiBの保守reserveを加算。
監視wallにpoll待ちが含まれるため、native /usr/bin/time -l real（丸めあり）と区別する。
