# QZT v0.1 Validation Corpus and Acceptance Thresholds

[English](QZT_v0.1_Validation_Corpus.md)

日付: 2026-06-08

## 目的

QZT をどの text に対して validation し、どの result を「expectations を満たす」と判定するかを
定義します。release hardening note は意図的に SLA を持たず metrics を記録しています。この note は
欠けていた layer、つまり corpus taxonomy と expectation bands を追加し、result を pass または
investigate と判断できるようにします。

criteria は 2 種類あります。

```text
- HARD invariant (MUST): every corpus, every run で成立する。違反は release blocker。
- SOFT target (SHOULD): expectation band。band 内なら "meets expectations"。band 外なら investigate であり、自動 blocker ではない。
```

これらの thresholds は product expectations であり、guaranteed SLA ではありません。project が change によって
product behavior を改善したか、regress したかを判断するために存在します。

## Corpus Taxonomy

QZT は AI memory systems の evidence を保存します。validation corpus は、そのような systems が実際に保持し
cite する text を model します。

```text
C1 conversation transcripts
   multi-turn dialogue, markdown-ish, moderate redundancy, UTF-8 with code blocks
   primary use case（spec Section 3 example）; doc_id は conversation; evidence_ref round-trip

C2 application and server logs
   timestamped repeated lines, ASCII-dominant, high redundancy
   high compression, bounded line access, rare-token search narrowing

C3 prose documents and knowledge base
   natural language, low redundancy, long paragraphs, markdown
   UTF-8 boundaries, doc_id ranges, n-gram substring search

C4 source code and structured data
   code, JSON, CSV; many short tokens; moderate redundancy
   byte-exact round-trip（whitespace and indentation preserved）, token search

C5 multilingual / CJK / emoji
   Japanese, Chinese, emoji; multi-byte UTF-8 throughout
   100% UTF-8 boundary safety, Unicode-scalar n-gram unit, boundary-safe range reads

C6 adversarial / pathological
   huge single line（no newlines）, near-random（incompressible）, extreme repetition, mixed CRLF/LF
   no panic, exact round-trip, incompressible upper bound, resource-limit enforcement
```

## Capability Expectations

各 corpus について、QZT capability ごとに HARD invariant と、該当する場合は SOFT target を定義します。

```text
lossless round-trip
  HARD: export(pack(x)) == x, byte-exact, all corpora
  SOFT: none（binary, always）

compression ratio
  HARD: whole-file zstd より catastrophically worse にならない
  SOFT: provisional target band: container overhead は whole-file zstd の +3..5% 以内; C2 < source の 15%; C3 < source の 45%

range restore
  HARD: requested bytes を正確に返す
  SOFT: provisional target band: decoded_bytes <= requested_size + 2 * chunk_size, file size 非依存

line access
  HARD: spanning lines を含め correct line bytes
  SOFT: provisional target band: line が跨る chunks に bounded; dense index は O(1) offset

rare-token search
  HARD: hit set equals ground truth
  SOFT: provisional target band: candidate decoded_bytes / source_bytes < ~1%

common-token search
  HARD: hit set equals ground truth
  SOFT: provisional target band: full candidate decode 前に capped

deep verify
  HARD: chunk / metadata / index の single-byte corruption を correct error で検出する
  SOFT: provisional target band: corruption sweep detection rate 100%, false-negative rate 0

evidence retrieval
  HARD: verified read は mismatch で fail closed し、wrong bytes を返さない
  SOFT: provisional target band: clean retrieval verified-match 100%; tampered retrieval failure 100%

search sidecar size
  HARD: rebuildable; rejection は Core read/export/verify を壊さない
  SOFT: provisional target band: token and n-gram sidecars 1.0..1.7x source

peak memory
  HARD: large input で OOM しない
  SOFT: provisional target band: range / line / verify peak <= max_chunk_size + index region, file size 非依存
```

## Per-Corpus Expectation Notes

```text
C1 conversation: compression 25..45% of source; doc_id per conversation resolves; evidence_ref restore is verified.
C2 logs: compression < 15% of source; rare-token search decodes far less than a raw scan.
C3 prose: compression 30..45%; UTF-8 safe; doc_id ranges restore exactly.
C4 code/structured: compression 20..40%; whitespace and indentation byte-exact; token search correct.
C5 multilingual: round-trip exact; no split Unicode scalar at any chunk or range boundary.
C6 adversarial: no panic; incompressible data stays near 1.0x plus bounded overhead; limits enforced.
```

## Mapping to Phases

```text
- Phase23a は C1..C6 の deterministic generators を構築し、non-evidence HARD invariants を assert し、SOFT targets を provisional expectation bands として記録する。
- Phase23b は Phase21 が verified evidence API を提供した後に evidence-retrieval HARD invariants を追加する。
- Phase18 は同じ generators を competitive timing に再利用し、large-size option を追加する。
- Phase21 は evidence-retrieval HARD invariants（clean 100%, tampered 100%）を検証する。
- Phase22 はこれら corpora の representative subset を portable golden vectors として freeze する。
```

## Reproducibility

```text
- corpora は deterministic and seeded; regeneration は byte-identical
- timing report は environment（CPU, OS, toolchain, tool versions）を捕捉する
- SOFT targets は evidence として記録される; out of band は flagged であり silent pass ではない
```

## Non-Goals

```text
- guaranteed SLA ではない
- competitive claims は Phase18 の領域であり、この note の領域ではない
- SOFT bands は provisional であり、Phase23a の first executable report と Phase18 の competitive benchmark evidence 後に見直す
```
