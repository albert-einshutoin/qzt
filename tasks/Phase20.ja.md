# Phase20: Public API Stabilization

[English](Phase20.md)

## 目的

QZT を Memory Pager のような embedder にとって安定した dependency にします。現状の `src/lib.rs` は
15 modules すべてを `pub mod` として公開しており、internal modules（`cbor`, `primitives`,
`fixed`, `schema`, `skeleton`）も含まれます。そのため、内部 refactor が downstream consumers にとって
breaking change になります。writer は near-duplicate な `pack_bytes_*` free functions を 8 個公開しており、
crate-level documentation はまだ Phase0 placeholders を説明しています。documented API-stability guarantee
もありません。

この Phase は public surface を curate し、writer entry points を consolidate し、public API を文書化し、
accidental surface growth に対する guard を追加します。container format bytes や
`export(pack(input)) == input` invariant は変更してはいけません。

## Minimum MVP

```text
- internal-only modules（cbor, primitives, fixed, schema, skeleton）は pub(crate) になるか curated surface の背後に隠れる
- public API は小さく意図的な set として lib.rs から pub use で re-export される
- 8 個の pack_bytes_* functions は writer builder の背後に consolidate される
- lib.rs crate-level documentation は Phase0 placeholder note ではなく real public API を説明する
```

## Goal MVP

```text
- public surface に #![warn(missing_docs)] があり、すべての public item が document される
- docs.rs metadata が設定され cargo doc が clean に build される
- semantic-versioning / API-stability policy が public/stable と internal を明記する
- public-API snapshot test（committed surface listing または cargo-public-api）が unintended surface changes で fail する
- 必要に応じて thin deprecated shims が 1 release だけ既存 caller を維持する
```

## Spec refs

```text
- format spec section はない。tasks/README.md Rust Style visibility guidance を参照する。
- この Phase は crate の Rust API surface だけを変更し、container format は変更しない。
```

## Conformance Tests Covered

```text
- 直接の conformance test はない。この Phase は downstream consumers を accidental breaking changes から守る
- writer builder は旧 pack_bytes_* output をすべて byte-for-byte で再現する
```

## TDD Plan

失敗する tests/checks を先に書きます。

```text
- public-API snapshot test は unintended item が public になると fail する
- writer builder は旧 pack_bytes_* variants それぞれと byte-identical output を生成する
- internal types（cbor, primitives, fixed, schema, skeleton）は crate 外から nameable ではなくなる
- missing_docs enabled で cargo doc --no-deps が warning なしになる
- CLI は curated public API だけを使って compile する
```

## Implementation Tasks

```text
1. 各 module/type を public API または internal として分類する
2. internal modules を pub(crate) にし、curated public types を lib.rs から re-export する
3. WriterOptions 上に pack_bytes_* variants を consolidate する WriterBuilder を設計する
4. external callers が存在する場合、1 release 分の thin deprecated wrappers を維持する
5. #![warn(missing_docs)] を追加し、全 public item を文書化する
6. Cargo.toml に docs.rs metadata を設定する
7. public-API snapshot test を quality gate に接続する
8. API-stability と semantic-versioning policy doc を書く
9. lib.rs crate-level documentation を書き直す
10. CLI を public API だけで動くように更新する（surface を dogfood する）
```

## Rust Notes

default は `pub(crate)` とし、`lib.rs` から deliberately small public surface を re-export します。
`WriterBuilder` は `WriterOptions` を所有し、profile、dictionary mode、Dense Line Index、
Document Index、container-id overrides を chained methods で expose します。public trait を downstream で
実装させたくない場合は sealed pattern を使います。CLI が public API だけを使うことは、その surface が
実際に十分であることの最も安価な証明です。

## Review Gates

この Phase を done にする前に code review を完了しなければなりません。

この Phase を done にする前に architecture review を完了しなければなりません。

どちらかの review で spec ambiguity や library constraint が見つかった場合は、続行前に
spec とこの phase plan を更新します。

## Self-Review Checklist

```text
- public surface は minimal で intentional か
- internal modules は crate 外から到達不能か
- writer builder は旧 pack_bytes_* output をすべて正確に再現するか
- 全 public item が document されているか
- accidental surface growth に対する automated guard があるか
- container format byte change を避けたか
```

## Done Criteria

```text
- internal modules が pub(crate) で、curated public surface が lib.rs から re-export されている
- WriterBuilder が pack_bytes_* sprawl を byte-identical output で置き換える
- missing_docs が enabled で、全 public item が document 済み
- docs.rs metadata が設定され cargo doc が clean に build される
- public-API snapshot test が quality gate に入っている
- API-stability と semver policy doc が存在する
- code review findings が修正済み
- architecture review findings が修正済み
- status.md が更新済み
```

## 状態

Complete。

完了日: 2026-06-08

実装範囲:

```text
- curated technical-preview API 用の WriterBuilder と crate-root re-exports を追加。
- internal modules は default で hidden とし、conformance tests 用に internal-testing feature のみで expose。
- API stability policy と docs.rs metadata を追加。
```

検証:

```text
- cargo test --all-targets --all-features --test phase20_public_api
- RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
- make check
```

Review notes:

```text
- Self-review pass 1 completed: WriterBuilder が legacy pack entry points を byte-for-byte に再現することを public API smoke test で確認。
- Self-review pass 2 completed: binary と example imports は internal module paths ではなく crate-root APIs を使用。
- Code review completed: low-level tests 互換性は internal-testing に隔離し、non-stable として文書化。
- Architecture review completed: default embedders には curated surface だけを見せ、reference implementation は conformance-test access を維持。
```

依存: Phase14（CI が doc / surface-snapshot gates を実行するため）。Phase21（integration examples が
stable public surface を使う）と Phase22（vector runner が public reader API だけを使う）の前提です。
Product Completeness Track の consumer sub-track を開始します。
