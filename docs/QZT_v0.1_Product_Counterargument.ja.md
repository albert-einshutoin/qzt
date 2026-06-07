# QZT v0.1 Product Counterargument

[English](QZT_v0.1_Product_Counterargument.md)

日付: 2026-06-07  
状態: adversarial critique

この文書は、現在の QZT product spec と phase plan に対する反論です。公平な評価ではなく、「実装できてもプロダクトとして成立しない可能性」を強く見るための文書です。

## 反対仮説

QZT は binary container として実装可能です。しかし難しい問いはそこではありません。

最も強い反論は次です。

```text
QZT は format problem を解いているが、価値のある user problem は retrieval、provenance、search、workflow integration にある。
その価値ある layer が Core の外にあるなら、Core format はよくテストされた低需要の storage primitive になる。
```

つまり、conformance では成功しても product として失敗する可能性があります。

## QZT が重要でないかもしれない理由

### 1. 主要価値が Core の外にある

Core が約束するのは lossless export、chunked zstd storage、byte range access、line access、verification です。これらは重要な engineering property ですが、AI memory、legal evidence、log analysis、archival retrieval の利用者が直接買う価値は ingestion、document identity、provenance metadata、search、ranking、access control、UI/API integration です。

Core が semantic search、ranking、summarization、vector DB behavior を除外することで設計はきれいになりますが、release されるものの product surface は小さくなります。

### 2. "Queryable" が名前ほど強くない

Core の query は byte read と line read です。発見や検索は optional sidecar または外部 system です。

```text
User expectation: compressed text を query できる
Core reality: 見たい位置を既に知っていれば byte range / line を読める
```

これは evidence replay には有用ですが、discovery ではありません。

### 3. 既存 tool で十分かもしれない

QZT は以下と競合します。

```text
- .zst + offsets manifest
- split zstd frames
- SQLite + FTS + BLOB chunks
- Tantivy / Lucene / Meilisearch / Elasticsearch
- Parquet / Arrow
- checksummed manifest 付き archive
- content-addressed blob + external search index
```

QZT が明確な優位を示せない限り、採用コストの低い既存 stack が勝ちます。

### 4. `.zst` 互換ではない

QZT は標準 zstd stream ではありません。これは container structure のために理解できますが、長期保管や運用では採用コストになります。

ユーザーは QZT reader を信頼する必要があります。cold evidence container である以上、長期 recoverability は特に重要です。

### 5. evidence ref だけでは source workflow にならない

QZT は byte range が container 内に存在することを検証できます。しかし、正しい bytes が参照されたか、source が完全だったか、memory system が evidence を正しく使ったかは保証しません。

document boundary、redaction、migration、normalized search hit と original bytes の対応などは外部 workflow の責務です。

### 6. immutability は living dataset と相性が悪い

Core は `finish()` 後 immutable です。更新には新 file、repack、merge、compact が必要になります。

active memory system、append-heavy logs、削除・修正・redaction が必要な user data では、QZT の周囲に別 system が必要になります。

### 7. sidecar が本当の product になる可能性

高性能 search は `.qzi` sidecar にあります。ならば問うべきは次です。

```text
なぜ sidecar は .qzt ではなく raw files、split zstd frames、content-addressed chunks、SQLite rows、object-store blobs を指してはいけないのか。
```

この問いに勝てなければ、QZT Core は置き換え可能な storage artifact になります。

### 8. verification が median user には過剰かもしれない

QZT の verification level、deterministic CBOR、checksums、strict offsets は優れた性質です。しかし evidence-sensitive ではない利用者には見えにくい価値で、単なる overhead に見える可能性があります。

### 9. 現在の benchmark は product を証明していない

現在の readiness note は smoke baseline であり、競合比較ではありません。packing speed、compression ratio、range access、index size、common query behavior を既存 stack と比較する必要があります。

## Product として不可能かもしれない理由

ここでの「不可能」は実装不能という意味ではありません。product goal が内部的に矛盾している、または運用上到達しにくいという意味です。

- Core は安定しているが、hard user problem を避けている
- exact evidence と normalized search は tension がある
- high-performance search は大きすぎる index を要求するかもしれない
- real data の document semantics が不足している
- sidecar-first architecture は mature search engine の問題を再発明する

## Phase ごとの反論

Phase0-9 は Core conformance として妥当です。一方で Phase10-13 は product risk が大きくなります。Dense Line Index、Document Index、token/ngram search、sidecar は、本当に QZT Core が必要なのかを競合 benchmark で証明する必要があります。

## Phase10 前に止まる強い理由

```text
1. Core の価値が adoption cost を上回る証拠がない
2. search/value layer が Core 外にある
3. sidecar が本体より重要になり得る
4. maintenance commands が未実装
5. 競合 benchmark gate がない
```

## Product kill criteria

以下が成立するなら、QZT は product として止めるべきです。

```text
- split zstd frames + manifest で同等の range access ができる
- SQLite/Tantivy/Lucene 連携の方が検索 UX と運用が良い
- sidecar size が許容できない
- evidence workflow が外部 system だけで解ける
- ユーザーが new binary format を受け入れない
```

## 反論を倒すために必要な実験

```text
1. 競合 range benchmark
2. 競合 search benchmark
3. sidecar size / latency benchmark
4. real corpus ingestion test
5. evidence workflow demo
```

## 推奨

QZT を続けるなら、次は機能追加ではなく product evidence を作るべきです。実 workflow または benchmark で、より単純な既存 stack より勝つことを示せる場合だけ、Phase10 以降の投資が正当化されます。
