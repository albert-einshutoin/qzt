# Phase14: Open-Source Release Hygiene

[English](Phase14.md)

## 目的

リポジトリを法務面・運用面で公開可能な状態にします。フォーマットと参照実装は
spec-complete ですが、LICENSE、自動検証、コントリビューター入口がないと、外部ユーザーは
採用・再配布・信頼できません。

この Phase はリポジトリのプロセスと metadata だけを変更します。コンテナフォーマットの挙動、
バイトレイアウト、`export(pack(input)) == input` の意味論は変更してはいけません。

## Minimum MVP

```text
- LICENSE が存在する（Rust ecosystem の標準である dual MIT OR Apache-2.0）
- Cargo.toml に公開 package metadata がある（license, description, repository, readme, keywords, categories, rust-version）
- GitHub Actions workflow が push / pull_request で `make check` を実行する
- .github ディレクトリ構造が存在する
```

## Goal MVP

```text
- CONTRIBUTING.md が tasks/README.md の TDD + dual review-gate 運用ルールを再掲する
- SECURITY.md が private disclosure contact と supported-version policy を記載する
- CI が stable と pinned MSRV の toolchain matrix を実行する
- CI が `cargo doc` を build し、doc warning を error にする
- `cargo package --allow-dirty` または同等の packageability check が通る
- crates.io publish dry-run は Phase20 で public API が安定するまで明示的に defer する
- release tag と CHANGELOG の規約を文書化する
```

## Spec refs

```text
- format spec section はない。tasks/README.md の運用ルールと Makefile quality gate を参照する。
```

## Conformance Tests Covered

```text
- 直接の conformance test はない。この Phase は既存の全 conformance test が every push で自動実行されることを保証する。
```

## TDD Plan

CI と metadata は Rust unit test ではなく、再現可能なコマンドで検証します。

```text
- `cargo package --allow-dirty` が missing-metadata error なしで成功する
- `cargo doc --no-deps` が warning なしで成功する
- pinned MSRV の `cargo build` が成功する
- CI workflow file が有効で、clean checkout で `make check` を green にする
- license-presence check（file が存在し Cargo.toml から参照される）が通る
```

## Implementation Tasks

```text
1. LICENSE-MIT と LICENSE-APACHE を追加し、Cargo.toml に license = "MIT OR Apache-2.0" を設定する
2. Cargo.toml の package metadata を埋める: description, repository, readme, keywords, categories, rust-version（u64::is_multiple_of のため MSRV >= 1.87）
3. .github/workflows/ci.yml を追加し、stable と MSRV で fmt、clippy -D warnings、test を実行する
4. doc warning を error として扱う cargo doc job を追加する
5. Phase contract と review gate を指す CONTRIBUTING.md を追加する
6. disclosure contact と supported versions を持つ SECURITY.md を追加する
7. v0.1 technical preview entry で CHANGELOG.md を seed する
8. `cargo package --allow-dirty` を検証する
9. crates.io publish dry-run は Phase20 public API stabilization 後まで defer する、と文書化する
10. release-tag convention を CONTRIBUTING.md または README に記載する
```

## Rust Notes

MSRV を明示的に pin し CI で検証することで、最小依存（blake3, zstd, proptest）の約束を
検証可能にします。現在のコードは `u64::is_multiple_of`（Rust 1.87 stabilized）を使っている
ため、MSRV は現代的です。依存が少ないことは古い toolchain build を意味しません。
`rust-version` は少なくとも 1.87 に設定します（より古い MSRV が必要なら該当 call site を
refactor する）。この Phase では新しい runtime dependency を追加しません。

## Review Gates

この Phase を done にする前に code review を完了しなければなりません。

この Phase を done にする前に architecture review を完了しなければなりません。

どちらかの review で spec ambiguity や library constraint が見つかった場合は、続行前に
spec とこの phase plan を更新します。

## Self-Review Checklist

```text
- 外部ユーザーが合法に fork / 再配布できるか
- CI が local `make check` gate を正確に再現しているか
- MSRV が宣言され、かつ test されているか
- 新しい runtime dependency が 0 か
- packageability check が crate metadata と included files の整合を証明しているか
- crates.io publish が Phase20 public API stabilization に明示的に gate されているか
- technical-preview status が明確で、production-ready と誇張していないか
```

## Done Criteria

```text
- LICENSE files が存在し、Cargo.toml license metadata と一致する
- CI が push / pull_request で make check green
- MSRV build job が通る
- cargo doc job が warning なしで通る
- cargo package または同等の packageability check が通る
- crates.io publish dry-run は post-Phase20 release gate へ defer されている
- CONTRIBUTING.md と SECURITY.md が存在する
- code review findings が修正済み
- architecture review findings が修正済み
- status.md が更新済み
```

## 状態

Complete。

完了日: 2026-06-08

実装範囲:

```text
- MIT/Apache-2.0 dual license、公開 Cargo metadata、MSRV、docs.rs metadata、CONTRIBUTING、SECURITY、CHANGELOG、CI を追加。
- crates.io publish は disabled のまま維持し、public API stabilization 後の release-owner 判断として gate を文書化。
```

検証:

```text
- make check
- RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
- cargo package --offline --allow-dirty
```

Review notes:

```text
- Self-review pass 1 completed: package metadata、license files、CI commands が release-hygiene done criteria と一致することを確認。
- Self-review pass 2 completed: publish は明示的に gate されたまま。crates.io DNS に到達できない sandbox のため、online dry-run は offline packageability で代替。
- Code review completed: release files は additive で、container bytes や CLI behavior を変更しない。
- Architecture review completed: public hygiene は crates.io publication から分離され、API stabilization 前の release を強制しない。
```

依存: なし。Product Completeness Track で最初に land すべき、最も低コストで効果の高い
product-readiness phase です。
