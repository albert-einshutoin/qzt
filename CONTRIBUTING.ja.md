# コントリビューションガイド

[English](CONTRIBUTING.md)

QZTは短命なfeature branchを使うGitHub Flowで開発します。変更は小さく、review可能にし、
`tasks/`のphase planまたは対象Issueへ結び付けてください。

## 開発契約

実装変更は次の順序で進めます。

```text
implement -> self-review -> code review -> architecture review -> fix -> verify -> update status
```

test、2回のself-review、code review、architecture review、指摘修正、
`tasks/status.md`更新が終わるまでphaseを完了扱いにしません。

## ローカル品質ゲート

CIと同じ品質ゲートを実行します。

```sh
make check
```

この既定ゲートにはwarningを拒否するrustdocも含まれます。生成HTMLはgitignore済みの
Cargo target directory内に留まります。

ドキュメントまたはrelease hygieneを変更した場合は、次も実行します。

```sh
make doc
cargo package --allow-dirty
```

`Cargo.toml`、`Cargo.lock`、依存ポリシーを変更する場合は、次も実行します。

```sh
cargo deny check bans licenses sources
```

このゲートは許可ライセンス、禁止・重複crate、依存の取得元を検査します。
既知の脆弱性は、CIのOSV Scannerが引き続き担当します。

CIのline coverageゲートをローカルで再現するには、`cargo-llvm-cov`を導入して
次を実行します。

```sh
make coverage
```

初回実測値は92.37%です。LLVMやplatform差のため約2ptの余裕を残しつつ、
大きな後退を拒否する初期基準として90%を設定しています。

公開exampleを変更する前に、利用者が通る実行経路を確認します。

```sh
cargo run --locked --example evidence_ref
```

parse、verify、fuzz targetを変更する場合は、`cargo-fuzz`を導入した環境で
bounded nightly smokeも実行します。

```sh
cargo +nightly fuzz run open_verify -- -max_total_time=60 -timeout=10 -max_len=4096
```

同じfuzz commandは毎週および手動dispatchで実行し、全PRでは実行しません。
生成corpusとcrash stateはローカルに残し、CI失敗時のcrash artifactは7日保持します。

## conformance testの追加

conformance testは`tests/`配下のintegration test binaryとして配置し、対応するphaseで
命名します。

```text
tests/phase{N}_*.rs
```

例は`tests/phase9_hardening.rs`と`tests/phase22_vectors.rs`です。実装対象の
`tasks/`およびphase planに合うphaseを選んでください。

### Core conformance map

[Core conformance item](docs/QZT_v0.1_Core_Spec.md#351-core-conformance-tests)
（item 1–77）の証拠になるtestを追加するときは、`tests/phase9_hardening.rs`の
`CORE_CONFORMANCE_MAP`も更新します。各entryは
`(item_number, description, evidence_test_name)`です。

`core_conformance_map_covers_all_items`はitem 1–77が順番どおりで、各証拠名が空で
ないことを検証します。Core itemを追加または番号変更した場合はmapを更新し、full
gateの前にhardening suiteを実行してください。

### 変更の検証

最初に変更箇所へ対応するfocused testを実行し、その後repository gateを実行します。

```sh
# phase9_hardeningは対象integration test binary名へ置き換える
cargo test --test phase9_hardening -- --nocapture

make check
```

`CORE_CONFORMANCE_MAP`を変更した場合は、証拠testが別phaseにあってもfocused commandへ
`phase9_hardening`を含めます。

## セキュリティ

脆弱性の疑いは[Security Policy](SECURITY.md)に従い、非公開で報告してください。
exploit details、secret、credentialをpublic Issueやpull requestへ記載しないでください。

CIはpull request、push、schedule、manual dispatchでSemgrep CE、OSV Scanner、
Gitleaksを実行します。選定基準と再利用可能なGitHub Actions templateは
`docs/Security_CI_Playbook.md`と`docs/Security_CI_Playbook.ja.md`を参照してください。

- SemgrepはRust rulesetのfindingをCI failureとして扱います。
- OSV Scannerは`Cargo.lock`の既知脆弱性を検査します。
- Gitleaksはgit history全体からsecret漏えいを検査します。

scan除外はfindingを確認したうえで最小範囲に限定し、理由をコメントで残してください。
tokenや個人環境の値をsource、Issue、PR、CI logへ含めてはいけません。

## release規約

annotated tagは`vMAJOR.MINOR.PATCH`形式を使います。QZT 0.1は公開後も
`technical preview`であり、production-readyとは表現しません。

crates.ioの実際の`cargo publish`は非可逆なrelease owner専用操作です。
準備、package review、dry-run、公開後確認は[release checklist](docs/RELEASE.md)に
従います。明示的なowner承認なしにpublishやstable tag作成を行わないでください。
