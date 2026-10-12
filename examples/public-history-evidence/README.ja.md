# 公開開発履歴の検索・原文照合例

Issue #344 / PR #345とcommit `3d092d1db64fdd3f0744377df98257038eddd32f`の公開snapshotを、検索から候補原文へ到達する限定例として使う。日常の再現はネットワーク不要。初回取得は[計画](PLAN.ja.md)の9 URL（API 7本・固定file 2本）だけで、[取得policy](snapshot/acquisition-policy.json)をGET前に保存した。本文10文書・36,633 bytes、response合計884,205 bytes。20文書/本文1 MiBの上限を使い切っていない。

可変Issue/PR本文は2026-10-12取得時点のsnapshotであり、過去時点の本文ではない。原本responseと抽出UTF-8 bodyは別fileで保持する。採用結果、具体的な回復理由、全問の不足は[RESULT](RESULT.ja.md)に記録し、実行物・入力・結果のhashは[canonical index](canonical-index.json)から追跡できる。

producer は snapshot の raw SHA-256、API の object ごとの `/body` または `/<index>/body`、固定ファイルの Git blob SHA-1 framingを照合する。`--source-sha` は自己申告で binary source の証明ではない。source を主張するには checkout、差分、toolchain、build command、binary SHA を含む外部 build receipt が必要である。pack 前に隣接 receipt を `x` 作成し、文書・questions・evaluation・policy・producer/binary/helper の hash を固定する。

同じsourceからのbuild再現は、対象checkoutでCore差分を確認してから行う。ここで得たbinaryは、同じversion文字列でも公開pre.6（source `0c8110e6b6e08513b4def3f636b5b79d068e275f`）とは別物である。

```sh
git diff --exit-code c11b7eb9a025f75d84ea9d3029a0674a606c04c6 -- src Cargo.toml Cargo.lock
rustc --version
cargo --version
cargo build --locked --bin qzt
shasum -a 256 target/debug/qzt
```

この手順とhashはcheckoutとbinary bytesの対応を再現する材料であり、署名、作者本人性、配布物identity、公開pre.6との同一性を保証しない。

```sh
trial=$(mktemp -d)
sha=$(python3 examples/public-history-evidence/produce.py \
  --binary "$PWD/target/debug/qzt" \
  --snapshot "$PWD/examples/public-history-evidence/snapshot" \
  --source-sha c11b7eb9a025f75d84ea9d3029a0674a606c04c6 \
  --output "$trial/handoff")
(cd "$trial/handoff" && python3 recipient.py --manifest-sha256 "$sha")
python3 examples/public-history-evidence/evaluate.py \
  --report "$trial/handoff/report.json" --manifest "$trial/handoff/manifest.json" \
  --evaluation examples/public-history-evidence/evaluation.json \
  --receipt "$trial/handoff-receipt.json" \
  --output "$trial/evaluation-report.json"
```

recipient は外部から渡された manifest SHA-256 を integrity chain として使うだけで、認証・署名とは呼ばない。必要なら `evaluate.py --report-sha256 ... --manifest-sha256 ...` に親shellの外部期待値を渡す。`deep verify` と index 構築は全 container を読む。その後、contextが一意に解決したhit文書だけを復元する。reportの `verified_documents` は、その文書のdoc_id、長さ、SHA-256、全bytesを保持する。evaluateはこれをmanifestへ独立再hashし、query、raw hit集合、global/local range、context excerptと同じ文書bytesを照合する。全文 export や検索前の全 document 復元はしない。QZI が既にあれば inspect し、破損時に再構築しない。

完全なsearchもpartial searchもCLI raw JSON全体をreportへ残す。既知のcap、既知incomplete、sidecarの`complete=false`、context budget/fragmentは候補bytesを捨てず `partial_unknown` として診断し、評価は全7問を出力する。未知reason、未知field、型違反、query順序・hit集合・文書bytesの不一致はErrorであり、評価outputを作らない。`index_coverage_verified=false`は通常の制約であり、0 hitを不存在証明へ変えない。

handoff は QZT、manifest、短い要約、questions、recipient、binary の6ファイルだけであり、evaluation、正解文書対応、正解 offset、expected hits を含めない。生成された binary/QZT/QZI は Git に入れない。

`handoff/`、`raw/final/`、`raw/fixed-20261012/`は原本を保護したlocal-onlyの旧試行で採用しない。初回q5の未取得report/commandsは`raw/initial-attempt/`に過去証拠として残し、[noncanonical index](noncanonical-index.json)で各旧fileのhash・所在・不採用理由を明示する。初回評価本文の完全復元はunknown。2026-10-12に許可されたq5の訂正はPLAN追補に記録し、初回の未取得を成功へ書き換えない。

この例は明示した検索語での byte correspondence を示すだけである。report内の文書bytesとchecksumだけでは、外部出典の真正性、意味、同意を証明しない。検索網羅性、意味的正解、第三者検証、実運用性能、production readiness、公開 release への収録、現在状態も保証しない。
