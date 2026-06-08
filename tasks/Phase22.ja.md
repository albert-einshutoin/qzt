# Phase22: Portable Conformance Vectors and Format Stability

[English](Phase22.md)

## 目的

QZT を 1 つの Rust crate ではなく、独立して検証可能な format にします。Cold evidence は長期可読性を
含意します。第三者、または将来の別言語 reader が、何年後でも `.qzt` file を validate できなければ
なりません。現状 conformance map は Rust test suite 内にあり、portable golden vectors として package
されていません。また frozen format-stability statement もありません。

この Phase は portable golden vectors、vector runner、third-party verification procedure、frozen v0.1
format-stability statement を commit します。container format bytes は変更してはいけません。ここでは
それらを freeze し document します。

## Minimum MVP

```text
- valid containers と representative corruption cases を cover する golden .qzt vectors を tests/vectors/ に commit する
- 各 vector の expected open/verify/export result を記述する manifest
- reference implementation を manifest に対して validate する vector-runner test
```

## Goal MVP

```text
- vector set は Core conformance map（open/verify/export/range/line）と corruption taxonomy を cover する
- third-party または other-language reader が vectors を実行できる procedure を文書化する
- frozen QZT v0.1 format-stability statement が、stable bytes/structures、forward/backward-compatibility policy、format_version negotiation を定義する
- vectors は deterministic に regenerate でき、regeneration command が文書化される
```

## Spec refs

```text
- Section 1.3 Core conformance and profiles
- Section 22 Immutability
- Section 34 conformance levels
- Section 35 test suite and conformance tests
- spec 全体の format_version handling
```

## Conformance Tests Covered

```text
- reference implementation は committed golden vector すべてを通す
- corruption vector は manifest の specified error variant で拒否される
- vectors は deterministic command から byte-identically regenerate される
- supported より新しい format_version を宣言する container は stability statement に従って reject または negotiate される
```

## TDD Plan

失敗する test を先に書きます。

```text
- vector runner は各 golden vector の open/verify/export/range/line result が manifest と一致することを assert する
- corruption vector は manifest の specified error variant で拒否される
- vector regeneration は deterministic: regenerate 後も byte-identical files になる
- unsupported newer format_version の container は stability statement に従って reject または negotiate される
```

## Implementation Tasks

```text
1. valid / corrupt .qzt files を生成する deterministic vector generator を作る
2. 各 vector の expected results manifest を書く
3. implementation を manifest に対して validate する vector-runner test を追加する
4. Core conformance map と corruption taxonomy を vectors で cover する
5. third-party / other-language verification procedure を文書化する
6. v0.1 format-stability and version-negotiation statement を書く
7. deterministic regeneration command を文書化する
```

## Rust Notes

vectors は小さく deterministic に保ち、git に入れても bloat しないようにします。generator は writer を
再利用します。runner は Phase20 public reader API だけを使うため、public-API smoke test も兼ねます。
stability statement は v0.1 container bytes を変更しないことに commit しなければなりません。byte layout
の変更は in-place edit ではなく、必ず新しい `format_version` です。

## Review Gates

この Phase を done にする前に code review を完了しなければなりません。

この Phase を done にする前に architecture review を完了しなければなりません。

どちらかの review で spec ambiguity や library constraint が見つかった場合は、続行前に
spec とこの phase plan を更新します。

## Self-Review Checklist

```text
- vectors は Core conformance map と corruption taxonomy を cover しているか
- 第三者は Rust source を読まずに .qzt を validate できるか
- runner は public reader API だけを使っているか
- v0.1 format-stability policy は明示的で frozen か
- vectors は byte-identically regenerate されるか
- container format byte change を避けたか
```

## Done Criteria

```text
- golden vectors と manifest が tests/vectors/ に commit 済み
- vector-runner test が manifest に対して通る
- corruption vectors が specified errors で拒否される
- third-party verification procedure が文書化済み
- v0.1 format-stability and version-negotiation statement が公開済み
- deterministic regeneration が文書化・検証済み
- code review findings が修正済み
- architecture review findings が修正済み
- status.md が更新済み
```

## 状態

Pending。

依存: Phase20（runner は stable public reader API を使う）、Phase23a（vector set は shared validation
corpora を再利用する）、Phase9 Core conformance map（完了）。format を independently verifiable かつ
long-term evidence readability のために frozen にします。
