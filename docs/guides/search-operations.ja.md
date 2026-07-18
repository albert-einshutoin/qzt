# QZT archiveの検索を運用する

**所要時間:** 15 minutes（約15分）  
**前提:** `qzt 0.1.0-pre.2`、`jq`。安定した契約は
[docs/CLI.ja.md](../CLI.ja.md)を参照してください。

再利用可能なtoken/ngram sidecarを構築し、resource capを適用し、返却hitと物理decode量を
混同せずにcostを読みます。

## 1. textをpackし、2種類のindexを作る

```sh
printf '%s\n' \
  '2026-07-19T03:00:00Z INFO tenant=alpha request_id=req-100 action=login status=200' \
  '2026-07-19T03:00:01Z ERROR tenant=alpha request_id=req-101 action=checkout incident=INC-9001 status=503' \
  '2026-07-19T03:00:02Z WARN tenant=beta request_id=req-102 action=checkout retry=1' \
  '2026-07-19T03:00:03Z ERROR tenant=beta request_id=req-103 action=payment incident=INC-9002 status=500' \
  '2026-07-19T03:00:04Z INFO tenant=alpha request_id=req-104 action=logout status=200' \
  | qzt pack - -o archive.qzt

qzt sidecar-rebuild archive.qzt --index token -o archive.token.qzi
qzt sidecar-rebuild archive.qzt --index ngram --ngram 3 -o archive.ngram.qzi
```

v0.1 builderはsourceをchunk単位でdecodeしますが、term dictionaryとposting map全体を
memoryに保持します。corpusに見合うmachineでbuildしてください。sidecarはderivedで
再構築可能であり、選択したQZTにbindしなければopen時に拒否されます。

## 2. tokenとn-gramを使い分ける

token検索はASCII letter/digitと`_`、`-`をtoken化し、ASCII letterをlowercaseへfoldします。
`ERROR`や`INC-9001`に向き、`error`も`ERROR`へmatchします。

```sh
qzt search archive.qzt ERROR \
  --sidecar archive.token.qzi --format json | jq
```

複数token queryは**co-occurrence**です。同じgranuleに全tokenがあればorderやadjacencyを
要求しません。phrase searchではありません。

n-gramは`n` Unicode scalar以上のsubstringに使います。

```sh
qzt search archive.qzt INC \
  --sidecar archive.ngram.qzi --format json | jq
```

sidecarの`n`はbuild時に固定されます。短いqueryはindex化できません。

```sh
qzt search archive.qzt IN \
  --sidecar archive.ngram.qzi --format json | jq
```

zero hitと`incomplete_reason="query_shorter_than_ngram_n"`を返し、stderrへwarningを出します。
non-null reasonがあるzero hitを完全な否定結果として扱わないでください。

## 3. candidate、decode、resultを制限する

```sh
qzt search archive.qzt ERROR \
  --sidecar archive.token.qzi \
  --max-candidates 100 \
  --max-decoded-bytes 16MiB \
  --max-results 1 \
  --format json > bounded.json

jq '{hits, capped, incomplete_reason, metrics: {
  candidate_granules: .metrics.candidate_granules,
  decoded_bytes: .metrics.decoded_bytes,
  physical_decoded_bytes: .metrics.physical_decoded_bytes,
  verified_matches: .metrics.verified_matches
}}' bounded.json
```

`--max-candidates`はcandidate granule、`--max-decoded-bytes`は検証するlogical candidate
byte、`--max-results`は返すhitを制限します。`capped=true`ならbudgetで処理または出力が
停止したため、省略hitをabsenceとして扱えません。`capped`と`incomplete_reason`は独立です。

tiny sampleでは1 hitと次のcostを観測しました。

```json
{"capped":true,"incomplete_reason":null,"metrics":{"candidate_granules":2,"decoded_bytes":104,"physical_decoded_bytes":452,"verified_matches":1}}
```

## 4. cost metricを正しく読む

- `candidate_granules`: byte検証前にposting listから得たcandidate数。
- `decoded_bytes`: 検証したlogical candidate byte。
- `physical_decoded_bytes`: 物理的に展開した完全QZT chunk。同一query内はcacheされます。
- `verified_matches`: original byteと照合できたoccurrence数。
- `index_size_ratio`: serialized index payload（granules、terms、postings）byte / source byte。
  QZI header/manifest overheadは含まれないため、on-disk全体は`wc -c archive.token.qzi`で別途測ります。

1 chunkしかない小さなfileでは、rare hit 1件でも`physical_decoded_bytes`がsource全体と同じに
なります。部分解凍の価値は、大きなarchiveの少数chunkだけにcandidateが触れる場合に現れます。
hitの`byte_length`はmatch spanであり、物理decode量ではありません。

## Limitations（制約）

- v0.1にphrase semantics、ranking、Unicode normalization、Unicode case folding、mutable updateは
  ありません。ASCII case foldingは実装済みです。
- sidecar buildは大きなmemoryを使い、sourceより大きくなる場合があります。
  [v0.1 benchmark report](../benchmarks/2026-07-v0.1.md)のtrade-offを確認してください。
- capは意図的にpartial resultを許します。`capped`、`incomplete_reason`、metricsを確認します。
- sidecarはtrusted evidenceではありません。authoritativeなQZT original byteで検証後にhitを返します。
- QZT/QZIはarchiveやquery artifactを暗号化しません。sensitive dataにはredaction、access control、
  storage/transport encryptionを適用します。

全コマンドはrelease binaryで実行済みです。
[tutorial validation record](tutorial-validation.md)を参照してください。
