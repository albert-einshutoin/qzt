# セキュリティ CI 選定ガイド

Date: 2026-06-16

このガイドは、QZT と他リポジトリに軽量なセキュリティチェックを横展開するための
再利用可能なベースラインです。最初は広く検知し、初回ベースラインをレビューしてから
失敗条件を強めてください。

## 各ツールの役割と選定基準

| ツール | 主な役割 | 入れる場面 | 選定基準 | 重複と注意点 |
| --- | --- | --- | --- | --- |
| Semgrep CE (`semgrep/semgrep`) | ソースコードの SAST とセキュアコーディングルール検査 | 対応言語のソースがあり、危険 API、バグパターン、プロジェクト固有ルールを PR 時に見たい場合 | Rust など単一言語なら `p/rust`、混在 repo なら `p/ci` から始める。コンテナ image は pin し、更新時だけ明示的に上げる。 | Semgrep Platform には secrets/SCA 系の機能もあるが、このベースラインでは CE を SAST として使う。全 severity を blocking にする前に false positive を確認する。 |
| Gitleaks CLI / `gitleaks/gitleaks-action` | working tree と Git 履歴の secrets 検出 | ほぼ全 repo。特に public OSS、infra、examples、tests、fixtures、設定ファイルが多い repo | GitHub Actions では `gitleaks-action`、ローカルや pre-commit では CLI を使う。履歴スキャンが必要なら `fetch-depth: 0` にする。 | SAST や依存関係スキャンの代替ではない。organization 所有 repo では `GITLEAKS_LICENSE` が必要になる場合があり、personal repo では不要。 |
| OSV Scanner / `google/osv-scanner-action` | lockfile、manifest、SBOM、必要に応じて image の依存関係脆弱性スキャン | `Cargo.lock`、`package-lock.json`、`pnpm-lock.yaml`、`yarn.lock`、`bun.lock`、`go.sum`、`poetry.lock`、Maven/Gradle ファイルなどがある場合 | cross-ecosystem の標準 SCA gate として使う。小さい repo は明示 lockfile 指定、monorepo は再帰スキャンを選ぶ。GitHub Code Scanning が使えるなら SARIF を upload する。 | JS/TS の依存関係検出は CVE Lite と一部重複する。OSV は広い ecosystem 用、CVE Lite は JS/TS の remediation 体験が必要なときだけ追加する。 |
| OWASP CVE Lite CLI | JS/TS 向けの依存関係脆弱性スキャンと修正ガイダンス | npm、pnpm、Yarn、Bun の project で、fix command、direct/transitive の見分け、SARIF、JSON、SBOM、offline advisory DB が欲しい場合 | JS/TS repo で `fail-on` severity threshold と実行可能な remediation が欲しいときに追加する。依存更新が多い repo では PR 前のローカル実行にも向く。 | Rust-only、Go-only、Python-only repo では入れない。JS/TS では OSV と検出範囲が一部重複するが、CVE Lite は修正手順の出し方が主な価値。 |

## 推奨レベル

| レベル | Semgrep | Gitleaks | OSV Scanner | CVE Lite CLI |
| --- | --- | --- | --- | --- |
| Baseline | PR と schedule で実行し、検出結果を確認して ignore を調整 | 明らかな新規漏えいを止める。test fixture は allowlist を狭く作る | まず検出。初回 dependency baseline を見るまでは non-blocking も可 | JS/TS repo で `fail-on: critical` または SARIF のみから開始 |
| Standard | suppression レビュー後に選定 ruleset で fail させる | required check にする。push と schedule では full history を見る | committed lockfile に対して `fail-on-vuln: true` | JS/TS repo で `fail-on: high` |
| Strict | custom rule と狭い suppression 運用を追加 | pre-commit や local CLI も追加 | image/SBOM scan と ignore 期限管理を追加 | 必要に応じて `--usage`、`--only-used`、offline DB、SARIF、SBOM を追加 |

## 判断ルール

- Git 履歴と PR を見る必須 secrets scanner が他にない限り、Gitleaks は全 repo に入れる。
- 対応言語で、初回 false positive をレビューできるなら Semgrep を入れる。
- lockfile または dependency manifest を commit する repo には OSV Scanner を入れる。
- CVE Lite CLI は npm、pnpm、Yarn、Bun lockfile を使う JavaScript/TypeScript repo のみに追加する。
- QZT のような Rust-only repo では Semgrep、Gitleaks、OSV Scanner を使い、CVE Lite CLI は省く。

## 再利用用 GitHub Actions サンプル

GitHub repo の汎用ベースラインです。Rust に特化した repo では Semgrep を `p/rust`、
OSV を `--lockfile=Cargo.lock` にし、`cve-lite-js` job は削除してください。

```yaml
name: security

on:
  pull_request:
  push:
  workflow_dispatch:
  schedule:
    - cron: "17 18 * * *"

permissions:
  contents: read

jobs:
  semgrep:
    name: semgrep
    runs-on: ubuntu-latest
    container:
      image: semgrep/semgrep:1.166.0
    steps:
      - uses: actions/checkout@v6
      - name: Run Semgrep CE
        env:
          SEMGREP_SEND_METRICS: "off"
        run: semgrep scan --config p/ci --error --metrics=off .

  osv:
    name: osv-scanner
    uses: google/osv-scanner-action/.github/workflows/osv-scanner-reusable.yml@v2.3.8
    permissions:
      actions: read
      contents: read
      security-events: write
    with:
      scan-args: |-
        -r .
      fail-on-vuln: true
      upload-sarif: true

  gitleaks:
    name: gitleaks
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0
      - uses: gitleaks/gitleaks-action@v3
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          # organization 所有 repo では必要。personal account の repo では不要。
          GITLEAKS_LICENSE: ${{ secrets.GITLEAKS_LICENSE }}
          GITLEAKS_ENABLE_COMMENTS: "false"

  cve-lite-js:
    name: cve-lite-js
    runs-on: ubuntu-latest
    permissions:
      contents: read
      security-events: write
    steps:
      - uses: actions/checkout@v6
      - name: Detect JS/TS lockfiles
        id: js-lockfiles
        shell: bash
        run: |
          if find . \
            -path ./node_modules -prune -o \
            -path ./target -prune -o \
            \( -name package-lock.json -o -name pnpm-lock.yaml -o -name yarn.lock -o -name bun.lock \) \
            -print -quit | grep -q .; then
            echo "present=true" >> "$GITHUB_OUTPUT"
          else
            echo "present=false" >> "$GITHUB_OUTPUT"
          fi
      - name: Run CVE Lite CLI
        if: steps.js-lockfiles.outputs.present == 'true'
        uses: OWASP/cve-lite-cli@v1
        with:
          path: "."
          verbose: "true"
          fail-on: high
          sarif: "true"
      - name: Upload CVE Lite SARIF
        if: always() && steps.js-lockfiles.outputs.present == 'true'
        uses: github/codeql-action/upload-sarif@v4
        with:
          sarif_file: ${{ github.workspace }}
```

## QZT/Rust 向けの差分

```yaml
semgrep:
  run: semgrep scan --config p/rust --error --metrics=off .

osv:
  with:
    scan-args: |-
      --lockfile=Cargo.lock
    fail-on-vuln: true

# JS/TS lockfile を追加するまでは cve-lite-js は省く。
```

## 運用上の注意

- action と container version は pin し、更新時だけ明示的に上げる。
- 初回 scan baseline を確認してから strict な blocking rule にする。
- allowlist や ignore は狭くし、なぜ許容するのかをコメントに残す。
- fork PR や repo 権限の都合で GitHub Code Scanning に書けない場合は SARIF upload を optional にする。
- untrusted code scan では、明確に hardening していない限り `pull_request_target` を使わない。

## 参考

- Semgrep: <https://github.com/semgrep/semgrep>
- Gitleaks: <https://github.com/gitleaks/gitleaks>
- Gitleaks Action: <https://github.com/gitleaks/gitleaks-action>
- OSV Scanner: <https://github.com/google/osv-scanner>
- OSV Scanner Action: <https://github.com/google/osv-scanner-action>
- OWASP CVE Lite CLI: <https://github.com/OWASP/cve-lite-cli>
