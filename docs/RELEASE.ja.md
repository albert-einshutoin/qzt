# QZT リリースチェックリスト

English: [RELEASE.md](RELEASE.md)

この手順書は、可逆な検証と非可逆な crates.io 公開を明確に分離して
QZT v0.1.0 を公開するためのものです。QZT v0.1 は本番対応製品ではなく、
technical preview として説明します。

次の[pre.4候補](releases/v0.1.0-pre.4-candidate.ja.md)は#318で扱う
**未公開**の準備です。公開Install/README導線はpre.3のまま維持します。
後日の公開判断には#318に記録するmerge済みsource SHAと候補証拠を使い、
以下の履歴上のpre.3公開コマンドをpre.4に流用しません。

## 公開済みpre.3 GitHub prerelease（2026-09-26）

[`v0.1.0-pre.3` GitHub prerelease](https://github.com/albert-einshutoin/qzt/releases/tag/v0.1.0-pre.3)は
製品commit `017d4d19739800773ab6a54adf636ff5a43ec1fc`から作った
technical previewです。crates.ioへの公開ではありません。
[#311](https://github.com/albert-einshutoin/qzt/issues/311)は未公開候補の
証拠を保持し、[release run](https://github.com/albert-einshutoin/qzt/actions/runs/36237333455)は
保護された`release` environmentの承認後に実公開assetを生成しました。
公開archive・checksum・native smoke・installerの証拠は
[#313](https://github.com/albert-einshutoin/qzt/issues/313)に記録します。

ownerはreview済みmerge commitを明示してannotated tagだけを作成・pushしました。

```sh
git tag -a v0.1.0-pre.3 017d4d19739800773ab6a54adf636ff5a43ec1fc -m "qzt v0.1.0-pre.3"
git push origin refs/tags/v0.1.0-pre.3
```

これは実行済みの履歴で、再実行や公開tagの移動を指示しません。
tag-onlyの[release workflow](../.github/workflows/release.yml)はmanifest版と
main ancestryを確認し、読み取り権限のbuild jobで4 native archiveとglobal assetを
生成し、保護environmentのreview後に限りwrite権限を持つhostが公開しました。
Releaseはprereleaseで、4 archiveと各sidecar、Unix/PowerShell installer、
source archive/checksum、集約checksum、dist manifestの14 assetを持ちます。

別の読み取り専用[公開物verifier](../.github/workflows/verify-published-release.yml)は
macOS ARM/Intel、Linux x64、Windows x64で実Release URLから取得し、
全assetのGitHub SHA-256 digestとarchiveの独立した**公開sidecarを照合してから**
展開binaryを実行し、digest照合済みの公開installerで隔離した一時先へ
導入します。[検証script](../scripts/verify-published-release.py)は小さな
[候補smoke](../scripts/verify-release-candidate.py)を再利用し、`dist build`は
繰り返しません。候補workflowはtag不存在を検査するため公開後に再実行しません。
製品source、release build、検証script commit、native runnerを別々に記録します。
release workflowは再buildするので候補と公開archiveのhashが異なる場合があります。
archiveとbinaryを別々に比較し、pre.2専用smoke・旧実測・旧attestationは
履歴として保持します。

## 公開権限

`publish = false`を削除する専用PRの承認と実際の`cargo publish`はrelease owner
だけが実行します。publish可能なstable manifestやdry-run成功はupload承認では
ありません。crates.io tokenをIssue、PR、端末ログ、CIログへ貼り付けてはいけません。

Issue #42では`publish = false`を維持した公開準備を証明しました。stable専用PRは
このguardを削除したpublish eligibilityを意図的に維持しますが、実uploadはその
正確なmerge commitからrelease ownerだけが行う別コマンドです。

## マージと公開を止める前提条件

- [ ] #22（公開pack APIの集約）がマージ済み
- [ ] #30（公開rustdocとlintの仕上げ）がマージ済み
- [ ] release ownerがcrates.io公開を明示承認済み
- [ ] リリースPR作成直前に`https://crates.io/crates/qzt`と
      `https://index.crates.io/3/q/qzt`の両方で`qzt`名が未使用であることを
      再確認し、競合時は別名を選ばずオーナーへエスカレーション
- [ ] `main`がcleanかつ最新で、必須CIがすべて成功
- [ ] バージョンが`0.1.0`で、technical preview表記を維持

可逆な候補準備とdry-runはowner承認前でも実行できます。一つでも未達ならstable
release PRをマージせず、実公開もしません。

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

stable専用release commitでmanifestが意図どおりpublish可能であることを確認し、
clean worktreeからdry-runします。

```sh
cargo metadata --no-deps --format-version 1
cargo publish --dry-run --locked
git status --porcelain
git diff --exit-code HEAD --
```

`qzt` packageのeffective metadataはversion `0.1.0`で、publicationのallow/deny
listがないことを確認します。このpublish eligibilityは意図したrelease入力なので、
dry-run後に`publish = false`へ戻しません。最後の2コマンドが何も出力せず、検証した
正確なcommitが変わっていないことを証明します。

Issue #42にはdry-run結果、ファイル数、packageサイズ、除外確認を記録し、
認証情報や無関係な環境情報は載せません。

## GitHub binary prerelease rehearsal

Issue #43はGitHubだけを使う可逆な予行であり、crates.ioへの公開を許可しません。
manifest versionは`0.1.0-pre.2`とし、この予行と他の全release前提が成功した後、
release ownerが承認する専用PRでのみstable版`0.1.0`へ戻します。

生成された`.github/workflows/release.yml`はtag pushだけをtriggerにし、branchや
pull requestでは起動しません。`make dist-check`はworkflowを再生成し、リポジトリの
最小権限化とdigest固定を再適用します。`release` environmentは
`v0.1.0-pre.*` tagだけを許可し、write権限を持つhost jobの前にrelease ownerの
reviewを要求します。Issue #43のPRをreviewしてmergeした後、その正確なmerge
commitへ予行tagだけを付けます。

```sh
git switch main
git pull --ff-only origin main
git status --short
git tag --annotate v0.1.0-pre.2 -m "qzt v0.1.0-pre.2"
git push origin v0.1.0-pre.2
```

- [ ] Releaseがprereleaseとして表示される
- [ ] `make dist-check`で生成workflowとhardeningが最新である
- [ ] release ownerが保護された`release` environmentへのdeploymentを承認する
- [ ] 4 targetのarchiveと各`.sha256` sidecarがある
- [ ] 展開したbinaryの`qzt --version`が`qzt 0.1.0-pre.2`を返す
- [ ] Linux artifactの動的依存が`libc.so.6`とRustのGNU unwind runtime
      `libgcc_s.so.1`だけで、zstdはbinaryへ静的linkされている

公開後は予行tagとReleaseを削除しません。これらは配布経路のimmutableな証跡です。
失敗時は公開済みtagを動かさず、新しいcommitとprerelease versionで修正します。

## オーナー承認制リリースPR

- [ ] CHANGELOGの`Unreleased`を`0.1.0 - YYYY-MM-DD`へ確定
- [ ] `publish = false`を削除
- [ ] 無関係なコード/API変更を含めない
- [ ] 品質ゲート、rustdoc、package一覧、dry-runを再実行
- [ ] 検証した正確なcommit SHAを記録
- [ ] マージ前にrelease ownerが明示承認
- [ ] 保護された`release` environmentへ正確な`v0.1.0` tag policyを追加し、
      release ownerのrequired reviewerを維持

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
