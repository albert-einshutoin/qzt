# Operate search over a QZT archive

**Time:** 15 minutes  
**Prerequisites:** `qzt 0.1.0-pre.2`; `jq`. The stable command contract is
[docs/CLI.md](../CLI.md).

This guide builds reusable token and n-gram sidecars, applies resource caps,
and reads search cost without confusing a returned hit with physical decode
work.

## 1. Pack text and build both index kinds

```sh
printf '%s\n' \
  '2026-07-19T03:00:00Z INFO tenant=alpha request_id=req-100 action=login status=200' \
  '2026-07-19T03:00:01Z ERROR tenant=alpha request_id=req-101 action=checkout incident=INC-9001 status=503' \
  '2026-07-19T03:00:02Z WARN tenant=beta request_id=req-102 action=checkout retry=1' \
  '2026-07-19T03:00:03Z ERROR tenant=beta request_id=req-103 action=payment incident=INC-9002 status=500' \
  '2026-07-19T03:00:04Z INFO tenant=alpha request_id=req-104 action=logout status=200' \
  | qzt pack - -o archive.qzt

qzt sidecar-rebuild archive.qzt --index token -o archive.token.qzi
qzt sidecar-rebuild archive.qzt --index ngram --ngram 3 -o archive.ngram.qzi
```

Build sidecars on a machine sized for the corpus: the v0.1 builder decodes the
source chunk by chunk but retains the complete term dictionary and posting map
in memory. Sidecars are derived, rebuildable, and rejected if they do not bind
to the selected QZT container.

## 2. Choose token or n-gram semantics

Token search recognizes ASCII letters/digits plus `_` and `-`, and folds ASCII
letters to lowercase. It is best for tokens such as `ERROR` or `INC-9001`;
`error` also matches `ERROR`:

```sh
qzt search archive.qzt ERROR \
  --sidecar archive.token.qzi --format json | jq
```

A multi-token query uses **co-occurrence**: all tokens must occur in the same
indexed granule, but order and adjacency are not required. It is not phrase
search.

N-gram search is useful for substrings of at least `n` Unicode scalar values:

```sh
qzt search archive.qzt INC \
  --sidecar archive.ngram.qzi --format json | jq
```

The sidecar's `n` is fixed at build time. A shorter query cannot be indexed:

```sh
qzt search archive.qzt IN \
  --sidecar archive.ngram.qzi --format json | jq
```

It returns no hits with
`incomplete_reason="query_shorter_than_ngram_n"` and writes a warning to
stderr. A non-null `incomplete_reason` means that no hits is not a complete
negative finding.

## 3. Bound candidates, decoding, and results

```sh
qzt search archive.qzt ERROR \
  --sidecar archive.token.qzi \
  --max-candidates 100 \
  --max-decoded-bytes 16MiB \
  --max-results 1 \
  --format json > bounded.json

jq '{hits, capped, incomplete_reason, metrics: {
  candidate_granules: .metrics.candidate_granules,
  decoded_bytes: .metrics.decoded_bytes,
  physical_decoded_bytes: .metrics.physical_decoded_bytes,
  verified_matches: .metrics.verified_matches
}}' bounded.json
```

`--max-candidates` limits candidate granules, `--max-decoded-bytes` limits
logical candidate bytes verified, and `--max-results` limits returned hits.
`capped=true` means work or output stopped at a budget; do not treat omitted
hits as absence. `capped` and `incomplete_reason` are independent signals and
both must be checked.

The validated tiny sample showed one returned hit and:

```json
{"capped":true,"incomplete_reason":null,"metrics":{"candidate_granules":2,"decoded_bytes":104,"physical_decoded_bytes":452,"verified_matches":1}}
```

## 4. Read the cost metrics correctly

- `candidate_granules`: posting-list candidates considered before byte
  verification.
- `decoded_bytes`: logical candidate bytes inspected by verification.
- `physical_decoded_bytes`: complete QZT chunks physically decompressed; a
  cache prevents counting the same chunk twice in one query.
- `verified_matches`: occurrences confirmed against original bytes.
- `index_size_ratio`: serialized index payload bytes (granules, terms, and
  postings) divided by source bytes. It excludes QZI header/manifest overhead;
  measure the complete on-disk cost separately with `wc -c archive.token.qzi`.

On a small one-chunk file, one rare hit can make `physical_decoded_bytes` equal
the entire source. Partial decompression becomes valuable when candidates touch
few chunks in a larger archive. The returned hit's `byte_length` is only the
matched byte span and must not be presented as physical decode work.

## Limitations

- Search has no phrase semantics, ranking, Unicode normalization, Unicode case
  folding, or mutable updates in v0.1. ASCII case folding is implemented.
- Sidecar construction can require substantial memory and the sidecar can be
  larger than the source. Review the measured trade-offs in the
  [v0.1 benchmark report](../benchmarks/2026-07-v0.1.md).
- Caps intentionally permit partial results. Check `capped`,
  `incomplete_reason`, and metrics before operational decisions.
- Sidecars are not trusted evidence. Hits are reported only after checking
  original bytes in the authoritative QZT container.
- QZT/QZI do not encrypt archive content or query artifacts. Apply redaction,
  access control, and storage/transport encryption for sensitive data.

The commands above were executed against the release binary. See the
[tutorial validation record](tutorial-validation.md).
