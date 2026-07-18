# QZT リリースチェックリスト

English: [RELEASE.md](RELEASE.md)

この手順書は、可逆な検証と非可逆な crates.io 公開を明確に分離して
QZT v0.1.0 を公開するためのものです。QZT v0.1 は本番対応製品ではなく、
technical preview として説明します。

## 公開権限

`publish = false` の削除承認と実際の `cargo publish` は release owner だけが
実行します。dry-run の成功は公開承認ではありません。crates.io token を
Issue、PR、端末ログ、CIログへ貼り付けてはいけません。

Issue #42では`publish = false`を維持したまま公開準備だけを証明します。
実公開は、オーナー承認を受けた別のリリースPRから行います。

## 公開を止める前提条件

- [ ] #22（公開pack APIの集約）がマージ済み
- [ ] #30（公開rustdocとlintの仕上げ）がマージ済み
- [ ] release ownerがcrates.io公開を明示承認済み
- [ ] リリースPR作成直前に`https://crates.io/crates/qzt`と
      `https://index.crates.io/3/q/qzt`の両方で`qzt`名が未使用であることを
      再確認し、競合時は別名を選ばずオーナーへエスカレーション
- [ ] `main`がcleanかつ最新で、必須CIがすべて成功
- [ ] バージョンが`0.1.0`で、technical preview表記を維持

一つでも未達なら公開作業を停止します。

## 可逆な準備

公開候補コミットのcleanなcheckoutで実行します。

```sh
git status --porcelain
make check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo package --list
cargo package --locked
```

最初のコマンドは何も出力しないことを確認します。default featureのrustdocが
公開APIの正本ゲートです。all-featuresは非公開conformance内部を含む追加の
compile確認であり、default featureのゲートを置き換えてはいけません。

package一覧には`Cargo.toml`、`Cargo.lock`、`README.md`、両ライセンス、
`CHANGELOG.md`、`src/`、移植可能なtests/vectorを含めます。`.github/`、
`fuzz/`、`tasks/`、英日Core Spec全文は含めません。ファイル数と圧縮後の
`.crate`サイズをリリースPRに記録します。

dry-run時だけ作業コピーから`publish = false`を外して実行します。

```sh
cargo publish --dry-run --allow-dirty --locked
git restore Cargo.toml
git status --porcelain
git diff --exit-code HEAD --
```

最後の2コマンドは何も出力しないことを確認します。これにより一時的なmanifest
変更が`Cargo.toml`だけでなく、追跡・未追跡のリリース入力全体を変更していない
ことを証明します。

Issue #42にはdry-run結果、ファイル数、packageサイズ、除外確認を記録し、
認証情報や無関係な環境情報は載せません。

## オーナー承認制リリースPR

- [ ] CHANGELOGの`Unreleased`を`0.1.0 - YYYY-MM-DD`へ確定
- [ ] `publish = false`を削除
- [ ] 無関係なコード/API変更を含めない
- [ ] 品質ゲート、rustdoc、package一覧、dry-runを再実行
- [ ] 検証した正確なcommit SHAを記録
- [ ] マージ前にrelease ownerが明示承認

## 非可逆な公開 — release ownerのみ

承認済みリリースPRの正確なmerge commitから実行します。

```sh
git switch main
git pull --ff-only origin main
git status --short
cargo publish --locked
```

- [ ] `cargo publish`が成功し、crates.ioに`qzt 0.1.0`が表示される
- [ ] docs.rsのbuildと公開API表示を確認
- [ ] 公開成功後、公開した正確なcommitへタグを付ける

```sh
git tag -a v0.1.0 -m "qzt v0.1.0"
git push origin v0.1.0
```

- [ ] #43のchecksum付きバイナリをGitHub Releaseへ添付
- [ ] #44でcrates.io/docs.rsの導線とbadgeを追加
- [ ] 新しい一時ディレクトリでinstall smokeを実行

```sh
cargo install qzt --version 0.1.0 --locked
qzt --version
```

公開後の同一versionは上書きできません。不具合時はyank方針に従い、
証跡を残して新しいpatch releaseを準備します。
