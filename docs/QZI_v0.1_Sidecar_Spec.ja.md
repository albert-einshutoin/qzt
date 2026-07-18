# QZI v0.1 Sidecar Spec

[English](QZI_v0.1_Sidecar_Spec.md)

日付: 2026-07-07

## 目的

この文書は、QZT v0.1 参照実装の Phase13 で提供される QZI（`.qzi`）検索 sidecar の公開 on-disk レイアウトを説明します。

QZI は QZT（`.qzt`）Core コンテナ上に構築される **派生・再構築可能・非信頼** の index です。Core コンテナ形式の一部ではありません。Core の意味論は [QZT v0.1 Core Spec](QZT_v0.1_Core_Spec.ja.md) 第 30 節を参照してください。

```text
.qzt = cold, immutable, verifiable evidence container
.qzi = hot, rebuildable search index bound to one source container
```

推奨ファイル名: `data.qzt.qzi`。

## 責務境界

| 関心 | QZT Core (`.qzt`) | QZI sidecar (`.qzi`) |
| --- | --- | --- |
| 証拠の整合性 | 正本 | 派生データ。source と一致必須 |
| read / export / verify | sidecar なしで常に利用可能 | 任意 |
| 検索候補の lookup | 不要 | granule / term / posting を提供 |
| ヒットの確定 | 原文バイトを decode して検証 | 単体では不十分 |

header、manifest、source binding、section 検証が通るまでは、sidecar の全バイトを非信頼として扱う必要があります。

sidecar の open / parse / search 失敗は **sidecar 経路のみで fail-closed** とし、Core の `open`、`export`、`verify`、range/line access は、sidecar が欠落・不一致・破損でも継続して動作しなければなりません。

## 物理レイアウト

```text
Offset  Size  Type   Field
0       8     bytes  magic = "QZISIDE1"
8       8     u64le  manifest_size
16      N     cbor   Sidecar Manifest (deterministic CBOR)
16+N    ...   bytes  section payloads (offset は byte 16+N 起点)
```

規則:

- `manifest_size` は deterministic CBOR manifest のバイト長です。
- manifest 内の section `offset` は、最初の payload バイト（manifest 直後）からの相対値です。
- 3 つの payload section は構築順に連続配置されます: `granules` → `terms` → `postings`。

## Sidecar manifest

トップレベル schema: `qzt.sidecar.v1`（旧形式）または
`qzt.sidecar.v2`（現行の compact writer 形式）。

必須フィールド（参照実装 `src/sidecar.rs` の encoder 名に一致）:

```yaml
schema: "qzt.sidecar.v2"
source_container_id: bstr16
source_format_version: [0, 1]          # 紐づく QZT コンテナの major, minor
source_original_checksum:
  algorithm: "blake3"
  value: bstr32
source_qzt_footer_checksum:
  algorithm: "blake3"
  value: bstr32
index_type: "token" | "ngram"
ngram_n: null | u64                    # index_type = "ngram" のとき必須。token は null
complete: bool
high_df_per_million: u32
granule_encoding: "legacy-v1" | "line-implied-v2"
term_encoding: "legacy-v1" | "key-posting-varint-v2"
index_manifest:
  schema: "qzt.search-index.v1"
  kind: string                         # index_type と同じ値
  posting_granularity: "line"
  index_size_bytes: u64                # payload 全体（granules + terms + postings）
  source_size_bytes: u64               # 紐づくコンテナの原文論理サイズ
sections:
  granules: { offset, size, checksum }
  terms:    { offset, size, checksum }
  postings: { offset, size, checksum }
```

各 section 参照:

```yaml
offset: u64                            # 最初の payload バイトからの相対 offset
size: u64
checksum:
  algorithm: "blake3"
  value: bstr32                        # section バイト列の BLAKE3-256
```

manifest CBOR は Core コンテナと同じ deterministic 規則（canonical map key order、integer width）を使います。

現行 writer は両方の encoding field を持つ `qzt.sidecar.v2` を出力します。
`qzt.sidecar.v1` は固定の旧レイアウトで両 field を省略し、現行 reader はその省略を
`legacy-v1` と解釈します。既存 v1 file は引き続き読み込めますが、compact v2 の
容量削減を得るには再構築が必要です。v1 専用 reader は v2 schema を拒否し、異なる
record layout として誤って decode してはいけません。

### Source binding

section を使う前に、reader は次を検証しなければなりません。

1. `schema` が `qzt.sidecar.v1` または `qzt.sidecar.v2` であること。
2. `source_format_version` が `[0, 1]` であること。それ以外は拒否（`UnsupportedVersion`）。
3. `source_container_id` が紐づくコンテナの `container_id` と一致すること。
4. `source_original_checksum` が紐づくコンテナ metadata の `original_checksum` と一致すること。
5. `source_qzt_footer_checksum` が、紐づくコンテナの Footer Payload（固定 trailer を除く footer バイト列）の BLAKE3-256 と一致すること。

不一致は sidecar を拒否します（`ContainerIdMismatch` または `ContainerCorrupt`）。Core の read/export/verify には影響しません。

### Index type 規則

- `index_type = "token"`: canonical writer は `ngram_n = null` を出力します。
  v0.1 参照 reader は互換性のため非負整数も受理しますが、token 経路では無視します。
  reader はこの値で token の意味論を変更してはいけません。
- `index_type = "ngram"`: canonical writer は正の整数を出力します。参照 reader は
  field の存在を要求し、0 では有効な n-gram search configurationを構成できないため、
  search はエラーになります。

上記以外の `index_type`、不正形式の `ngram_n`、または n-gram 経路での
`ngram_n` 欠落は sidecar を拒否します。

### High-document-frequency threshold

`high_df_per_million` は planner の inclusive threshold
`granule_frequency * 1_000_000 / granule_count` です。参照 writer は build option を
manifest に記録し、既定値は `200_000`（20%）です。現行 CLI からは変更できません。
n-gram search では threshold 以上の key を high-DF とし、必要な key がすべて
high-DF なら早期に capped result を返して、common term の無制限な candidate decode
を避けます。

## Section payload

reader は decode 前に、各 section の `offset`、`size`、`checksum` を sidecar ファイル境界に対して検証しなければなりません。checksum 不一致や範囲外アクセスは sidecar を拒否します。

### `granules` section

バイナリレイアウト:

```text
u64le granule_count
```

`granule_encoding = "legacy-v1"` は従来の 56-byte record を使います。

```text
granule_count 回繰り返し（各 56 バイト）:
  u64le granule_id
  u64le logical_offset
  u64le byte_length
  u64le chunk_start
  u64le chunk_end
  u64le first_line      # u64::MAX は欠落
  u64le line_count      # u64::MAX は欠落
```

`granule_encoding = "line-implied-v2"` は canonical line index に 20-byte の
固定長 record を使います。

```text
granule_count 回繰り返し（各 20 バイト）:
  u64le logical_offset
  u32le byte_length
  u32le chunk_start
  u32le chunk_span                 # chunk_end - chunk_start
```

`line-implied-v2` では `granule_id` と `first_line` は 0 始まりの record index、
`line_count` は 1 です。固定長のまま暗黙値を省略するため、file-backed random lookup
の O(1) 性を維持できます。

期待 section サイズは `8 + granule_count * record_size` です。`record_size` は
`legacy-v1` が 56、`line-implied-v2` が 20 で、不一致は拒否します。

各 granule record は、source コンテナ内の論理バイト範囲と chunk span への posting ターゲットを表します。

### `terms` section

`term_encoding = "legacy-v1"` は従来の固定 field layout を使います。

```text
u64le term_count
term_count 回繰り返し:
  u64le key_len
  key_len bytes key
  16 bytes key_hash
  u64le document_frequency
  u64le granule_frequency
  u64le posting_offset    # postings section 先頭からの相対 offset
  u64le posting_size
  u64le skip_offset
  u64le skip_size
  u64le flags
```

`term_encoding = "key-posting-varint-v2"` は compact v2 layout を使います。

```text
u64le term_count
term_count 回繰り返し:
  varuint key_len
  key_len bytes key
  varuint granule_frequency
  varuint posting_size
```

v2 では `key_hash` を `key` から再計算し、`posting_offset` はそれ以前の
`posting_size` の累積値です。document / skip / flags field は 0 とします。
これにより、unique log token ごとの 80-byte 固定 envelope を省きながら、
sorted binary lookup と改ざん検査を維持します。

term は `key` で厳密に昇順でなければなりません。`key_hash` は正確な raw `key`
bytes に対する BLAKE3-256 の先頭16 bytes と一致し、`flags` は 0 でなければなりません。posting range は 0 から
連続し、末尾が postings section size と完全一致しなければなりません。これらを
検証することで、reader は未検証の辞書順序を信頼せず binary search できます。

`legacy-v1` の `skip_offset` と `skip_size` は QZI v0.1 では予約済みの planner metadata です。
serialized skip-data section は存在せず、これらの値にfile offsetの基準点はありません。
reader はこれらを使って seek してはいけません。参照in-memory readerはdecodeした
posting listからskip pointを再構築し、file-backed readerはこれらのfieldを無視します。
永続化したskip dataを利用するには、将来のsidecar versionで新しいpayload contractを
定義する必要があります。

term key はソート済みです。`key_hash` は lookup 補助であり、exact `key` 比較は必須です。

### `postings` section

posting list の連結です。各 term の `[posting_offset, posting_offset + posting_size)` は、1 つのソート済み granule ID 列を `delta-varint-u64-v1` で符号化します。

```text
first granule_id as unsigned varint
then (granule_id - previous_granule_id) as unsigned varints
```

例えば granule ID `[1, 2, 100]` は、最初の絶対値 `1`、続く delta `1` と `98` として
`0x01 0x01 0x62` に encode します。decode 後の ID は厳密な昇順でなければなりません。

posting list は `granule_id` の昇順でなければなりません。参照される `granule_id` はすべて `granule_count` 未満でなければなりません。

## 検索と検証の責務境界

sidecar のヒットは **候補** に過ぎません。search は次を行います。

1. query key の posting list を intersect する。
2. 候補 granule を chunk span に解決する。
3. **source QZT コンテナ** から重なる原文バイトを decode する。
4. その原文バイトに対して token または n-gram 規則で一致を検証する。

sidecar 単体を内容の証拠として扱ってはいけません。section checksum を通過してもコンテナと矛盾する改ざん sidecar は、原文検証または open 時の source binding で失敗します。

## Fail-closed まとめ

sidecar 経路は次を拒否しなければなりません（非網羅）。

| 条件 | 典型エラー |
| --- | --- |
| magic 不一致または header 切り詰め | `InvalidHeader` / `UnexpectedEof` |
| 非 deterministic または未知 manifest schema | `ContainerCorrupt` |
| 未対応 `source_format_version` | `UnsupportedVersion` |
| `source_container_id` 不一致 | `ContainerIdMismatch` |
| `source_original_checksum` 不一致 | `ContainerCorrupt` |
| `source_qzt_footer_checksum` 不一致 | `ContainerCorrupt` |
| section 範囲外 | `UnexpectedEof` |
| section checksum 不一致 | `ContainerCorrupt` |
| granule / term / posting parse 失敗 | `ContainerCorrupt` |
| 境界算術の整数 overflow | `ResourceLimitExceeded` |

これらの失敗は、紐づく `.qzt` に対する Core 操作を妨げてはいけません。

## 再構築と運用

sidecar は任意かつ再構築可能です。

```text
qzt sidecar-rebuild input.qzt -o input.qzt.qzi
qzt search input.qzt "query" --sidecar input.qzt.qzi
```

sidecar が無い場合は、コンテナから一時的な in-memory index を構築できます。Core 動作は sidecar の有無に依存しません。

## 参照実装

Phase13 参照コード: `src/sidecar.rs`、`tests/phase13_sidecar.rs`。

sidecar 拒否と Core 分離の conformance は `tests/phase13_sidecar.rs` で検証されています。

## 関連文書

- [QZT v0.1 Core Spec](QZT_v0.1_Core_Spec.ja.md) — Core コンテナ形式と Search Extension 概要（第 29–30 節）
- [QZT v0.1 Format Stability](QZT_v0.1_Format_Stability.md) — Core format stability statement
