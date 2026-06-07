# Fixture strategy

[English](README.md)

Fixture は trust level ごとに分けます。

```text
source/   valid QZT container を作るための original UTF-8 input
valid/    source fixture から生成された well-formed QZT container
corrupt/  parser / verifier tests 用の intentionally malformed container
```

後続 Phase では、binary fixture は小さく、deterministic で、その fixture を使う test から目的が分かる状態にします。
