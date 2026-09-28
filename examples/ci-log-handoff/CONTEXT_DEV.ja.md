# 公開pre.5証跡の開発版context追試

これは[公開pre.5のcase study](CASE_STUDY.ja.md)の後続検証であり、新しいrelease
計測ではない。QZTとjob log原本byteは#325のまま保持した。`qzt context` binaryは
Issue #327の開発commit `27203c5493ee8f2e036655ab39d9a07b8c9b0c42`から
buildした。公開`v0.1.0-pre.5`と製品source
`3bc7d2561c58b59cd166fed75bfbd45f2f4c0ebe`には新コマンドは含まれない。

受取用の一時directoryには**`evidence.qzt`だけ**をcopyした。そのQZTへの
`search --index ngram --ngram 3 --format json`が非打切りhitを返し、
`logical_offset`・`byte_length`をそのまま`context`へ渡した。新コマンドに
原本、`source.json`、`queries.json`、文書開始位置の引き算、QZIは不要。

```sh
qzt search evidence.qzt 'build environment unchanged:' --index ngram --ngram 3 --format json
qzt context evidence.qzt --offset 65344 --length 28 --before 2 --after 2 --format json
```

後者は`mapping_status=unique`、文書ID`ci/pre5-linux.log`、文書内hit
`31281:31309`、返却した原文の全体範囲`65192:65649`（457 byte）を返した。
前後の停止理由はいずれも`complete`。`bytes_hex`は復元可能で、
escaped textは表示専用。

| query / 出現箇所 | 全体hit | 文書内hit | 原文excerpt全体範囲 |
|---|---:|---|---:|
| dirty-checkout error | 32023:32060 | pre4 32023:32060 | 30916:32182 |
| unchanged environment | 65344:65372 | pre5 31281:31309 | 65192:65649 |
| dist、pre4 echo | 25026:25047 | pre4 25026:25047 | 24703:25144 |
| dist、pre4実出力 | 29906:29927 | pre4 29906:29927 | 29555:30162 |
| dist、pre5 echo | 59090:59111 | pre5 25027:25048 | 58759:59239 |
| dist、pre5実出力 | 64001:64022 | pre5 29938:29959 | 63650:64257 |
| 新規`Traceback (most recent call last):` | 31249:31283 | pre4 31249:31283 | 31097:31447 |

`pre4`・`pre5`はそれぞれ`ci/pre4-linux.log`・`ci/pre5-linux.log`の略。
全行とも`before.stop=complete`・`after.stop=complete`。dirty errorは
`--before 12 --after 2`でTraceback、`snapshot` frame、raise文、
exit code 1の行を取得した。他は前後2行。`dist ran successfully`の4 hitには
echo commandと実出力が各2件あり、成功記録の自動判定は行わない。

`cargo test --locked --features internal-testing --test phase56_context_cli
--test phase56_context_case_study`で追試できる。case-study testは追跡原本を
独立にbyte走査してhit期待値を作り、返却excerptを原本sliceと照合する。
原本はテストのoracleだけに使い、受取用directoryと`context` processには
QZTのみを渡す。

これは部分読取の証拠である。展開したchunkと保存Document Index blockは
検証するが、文書全体checksum、検索index網羅性、job metadata、外部出典の
真正性は確認しない。以前のpre.5 raw記録と計測値は公開binaryの履歴として
保持する。
