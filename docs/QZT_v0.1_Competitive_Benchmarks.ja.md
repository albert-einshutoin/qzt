# QZT v0.1 競合ベンチマークハーネス

日付: 2026-06-08

[English](QZT_v0.1_Competitive_Benchmarks.md)

Phase18ハーネスは再現可能な計測であり、SLAではありません。Phase23のvalidation
corpus generatorを使い、同一byteに対するQZTのrange restoreとwhole-file raw zstd
restoreを比較します。

最新の計測reportと完全なraw runは
[docs/benchmarks/2026-07-v0.1.md](benchmarks/2026-07-v0.1.md)で公開しています。

## QZTを使う場合

QZT v0.1はtechnical previewです。database型indexやwhole-file decompressionより、
検証済みoriginal-byte evidenceが重要な場合に利用してください。

| Workload | QZTを使う条件 | 注意点 |
| --- | --- | --- |
| Evidence container | source byteとintegrity checkを保持するcoldでimmutableなcontainerが必要 | Technical previewであり、production SLAや性能保証はない |
| 大規模immutable log | append-once textを再圧縮やarchive全体のdecodeなしでseek可能にしたい | benchmark timingは再現可能なevidenceであり、性能の約束ではない |
| Byte-exact range restore | compressed storageからfull-file decodeではなく検証済みsliceが必要 | restoreは要求rangeと重なるchunkだけをdecodeする |
| 検証済みdocument retrieval | stable IDをoriginal-byte rangeへ解決し、partial decodeでround-tripしたい | Document Indexはoptional extensionであり、original textの代替ではない |
| 再構築可能なsidecar検索 | 検索を派生QZI indexに置き、hitをcontainer byteへ再照合できる | QZI sidecarは再構築可能かつoptionalで、source textより大きくなる場合がある |

既定のsmokeを実行します。

```sh
cargo test --features internal-testing --test phase18_competitive_benchmark -- --nocapture
```

`internal-testing`は、このfocused integration testにだけ非公開のbenchmark/corpus
harnessを公開し、QZTのdefault public APIは変更しません。

SQLite FTS5とripgrepを使う外部tool比較は`bench-compete` featureの背後で実行し、
既定のquality gateをportableに保ちます。見つからないtoolはskipします。利用可能な
toolはreference byte scanと同じhit数を返さなければなりません。

```sh
cargo test --release --all-features --test phase18_competitive_benchmark -- --nocapture
```

`--all-features`は、このintegration harnessがimportするinternal test surfaceとともに
`bench-compete`を有効にします。

## 方法

- 明示的なseedからdeterministicなC1-C6 corpus byteを生成する。
- 固定chunk sizeでQZTへpackする。
- 同じcorpusをwhole-file zstdで圧縮する。
- 同じbyte rangeを`QztFileReader`とwhole-file zstd decodeでrestoreする。
- timingを記録する前にbyte equalityをassertする。
- sidecar/search correctnessはtimingと分けて記録する。
- `bench-compete`有効時は同じcorpusにripgrepとSQLite FTS5を実行し、reference byte
  scanとhit数が一致しなければharnessを失敗させる。

## QZTを使わない場合

mutable ranking、normalized language search、または高頻度更新indexが主なworkloadなら、
QZTはfull-text databaseの代替ではありません。QZI sidecarは再構築可能でsource text
より大きくなる場合があるため、index compactnessより検証済みoriginal-byte evidenceが
重要な場合に使ってください。
