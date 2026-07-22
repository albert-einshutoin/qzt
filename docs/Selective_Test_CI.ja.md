# 選択的テストCI

QZTは「test数を減らすこと」ではなく、失敗検出能力を維持したままPull Requestの
実行時間と計算量を減らすことを目的にします。影響範囲を安全に限定できる場合だけ
選択実行し、それ以外は全testへフォールバックします。

English: [Selective_Test_CI.md](Selective_Test_CI.md)

## CIレーン

| event | test戦略 | 継続するgate |
| --- | --- | --- |
| `main`向けPull Request | 影響対象。判定不能時は全件 | format、Clippy、全target compile、docs、package、依存policy、Windows release build、security scan |
| `main` / `release/**`へのpush | 全件 | coverageを含む全gate |
| 日次schedule | 全件 | coverageとsecurity scan |
| manual dispatch | 全件 | coverageを含む全gate |

PRでcoverageを実行すると全testをもう一度走らせるため、coverageはpost-merge、release、
日次の完全検証へ移します。選択testはstableで実行し、stableとMSRVの両方で全targetを
compileするため、version互換性を選択結果へ依存させません。

## 構成と判定

- `ci/impact.py`: CI serviceに依存しない共通層。差分、依存graph、plan、実行、logを担当
- `ci/config/impact.json`: 全件rule、safe path、smoke、E2E、手動mappingを集約
- `ci/adapters/rust.json`: Cargo検出、command、cache、source/test glob、Rust固有の危険変更
- `ci/adapters/rust.py`: Rust module、import、public re-exportの解析

plannerは明示されたbase/headを検証し、NUL区切りの`git diff --name-status -z -M -C`
から追加・変更・rename・copy・削除を読みます。危険変更を先に判定した後、
`crate::module`からmodule依存graphを作り、変更moduleを使う上位moduleまで逆向きに
辿ります。qualified import、public re-export、直接変更されたtest、手動mappingを
統合し、最後に常時smokeを追加します。

QZTではtargetを直列実行します。Cargoは同じbuild directoryを共有するため、hosted
jobを細分化するとcompileが重複し、wall timeが短くても課金対象の総計算量が増える
可能性があるためです。planはcategory別JSON配列なので、実測で有利と確認できたrepoは
上限付きmatrixへ拡張できます。

## 安全側フォールバック

依存manifest/lock、CI・build・test設定、共通format/schema/error/I/O/limit、fuzz設定、
revision不足、Git差分失敗、未分類path、依存graph失敗、impact設定破損、存在しない手動
mapping targetでは全testを選びます。adapter自体が壊れた場合は信頼できる全test
commandを復元できないため、CIを失敗させます。

削除pathは`deletedFiles`へ残しますが、runnerには現在存在するtestだけを渡します。
selectorやrunnerの失敗を「testを実行せず成功」へ変換しません。

## 証跡とローカル実行

workflowはplanと実行summaryを14日保持します。base/head、変更・削除file、影響module、
test category、fallback理由、target単位の成功・失敗・skip、所要時間を確認できます。

```sh
python3 ci/impact.py plan \
  --repository . --base origin/main --head HEAD \
  --config ci/config/impact.json --adapter ci/adapters/rust.json \
  --output target/ci/impact-plan.json

python3 ci/impact.py run \
  --repository . --plan target/ci/impact-plan.json \
  --adapter ci/adapters/rust.json --summary target/ci/test-summary.json
```

planner、policy、adapter、workflowを変更したら`make ci-test`を実行してください。
CLI、生成物、document契約などstatic importで関係を表せないtestは手動mappingへ追加します。
