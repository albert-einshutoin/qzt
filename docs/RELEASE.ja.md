# QZT リリースチェックリスト

English: [RELEASE.md](RELEASE.md)

この手順書は、可逆な検証と非可逆な crates.io 公開を明確に分離して
QZT v0.1.0 を公開するためのものです。QZT v0.1 は本番対応製品ではなく、
technical preview として説明します。

## 現行pre.3候補の準備（未公開）

Issue #311ではpackage version `0.1.0-pre.3`のGitHub prerelease候補
`v0.1.0-pre.3`を準備します。tag、Release、crates.io公開は許可しません。
[候補ノート](releases/v0.1.0-pre.3-candidate.ja.md)と
[pre.2からの移行](releases/v0.1.0-pre.3-migration.ja.md)は公開済みpre.2 tag
からの変更を説明します。下のpre.2とstableの節は過去の記録または将来のowner gateで、
そこにあるversion commandは今回の候補準備に使いません。

cleanな候補commitから、読み取り権限だけの
[`release-candidate.yml`](../.github/workflows/release-candidate.yml)で
`dist plan`、4 targetのnative `dist build --artifacts=local`、global buildを
行います。verifierは各archiveのSHA-256 sidecarを照合し、展開したbinaryを
対象OS/architectureで小さなE2E smokeに通します。PR runは予行です。
merge後にはmainの正確なmerge SHAを指定して同workflowをdispatchし、14日保持の
artifactを#311へ記録します。将来のinstaller downloadや公開assetのbyte一致は
この予行では証明できません。

## pre3候補のowner承認後の公開

これは#311に正確なmain merge SHA、通常・security CI、4 targetの証拠を
記録した**後日のowner操作**です。tagとReleaseが未使用であることを確認し、
検証済みSHAがHEADのclean checkoutでownerが不変のannotated tagを作成・push
できます。

```sh
git fetch origin main --tags
git switch --detach <検証済みの完全なmerge SHA>
git status --porcelain
git rev-parse HEAD
git merge-base --is-ancestor HEAD origin/main
git ls-remote --tags origin refs/tags/v0.1.0-pre.3
git tag -a v0.1.0-pre.3 -m "qzt v0.1.0-pre.3"
git push origin v0.1.0-pre.3
```

`ls-remote`は空でなければ停止します。保護されたtag-onlyの
[release workflow](../.github/workflows/release.yml)がtagの版とmain ancestryを
検査し、write権限を持つhost jobの前にrelease ownerが保護された`release`
environmentを承認します。承認設定を回避・緩和しません。このGitHub previewに
`cargo publish`は含めません。

公開後、実Releaseがprereleaseであること、4 archive、対応する4つの`.sha256`、
installer、source archive/checksumが揃うことを確認します。**公開された実asset**
を新規directoryへ取得し、checksumを照合し、各native OS/architectureで
展開binaryを実行して`qzt --version`が`qzt 0.1.0-pre.3`であることと
[候補smoke](../scripts/verify-release-candidate.py)相当の動作を確認します。
Linux linkageとinstallerの実downloadもURLが利用可能になってから検査します。
release workflowはtagから再buildするため、公開archiveのhashは候補CIと
同じbyte列とは限りません。公開hashとrun IDを記録した後、README Install/tourの
linkを別の公開後変更で更新します。pre.2の証拠は残します。

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
