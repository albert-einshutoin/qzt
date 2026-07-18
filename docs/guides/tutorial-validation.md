# Tutorial validation record (2026-07-19)

The three English/Japanese tutorial pairs use the same commands. This record
captures the release-binary validation behind their abbreviated output.

## Environment

- Source commit: `0705c74c2a1e683e9b2a2e2a7c21935b0e11e990`
- Binary: `qzt 0.1.0-pre.2`, release build
- Host: macOS 26.5 (25F71), arm64
- JSON processor: `jq 1.7.1-apple`
- Date: 2026-07-19 JST

## Executed journeys

All commands exited `0`, including the intentionally incomplete two-scalar
n-gram query, which writes its warning to stderr.

### Log preservation

1. `qzt pack - -o daily.qzt < daily.log`
2. `qzt attest daily.qzt > daily.attest.json`
3. Deep-coverage `jq -e` policy check
4. `qzt verify daily.qzt --deep --format json`
5. Token `sidecar-rebuild`, JSON search for `INC-4242`, and `range` using the
   returned `logical_offset` plus `byte_length`

```text
verify: {"ok":true,"level":"deep","checked_chunks":1,"decoded_bytes":288}
hit: logical_offset=200 byte_length=8 source=verified_original_bytes
metrics: decoded_bytes=87 physical_decoded_bytes=288 verified_matches=1
range stdout: INC-4242
```

### Artifact fixation

1. `qzt pack-docs report.txt metrics.csv run.log -o run-1234.qzt`
2. `qzt attest run-1234.qzt > run-1234.attest.json`
3. `qzt docs run-1234.qzt --format json`
4. Verified `qzt doc`, byte-for-byte `cmp`, regenerated attestation, and empty
   `diff -u`

Observed document IDs and byte spans were `report.txt` at `0+58`,
`metrics.csv` at `58+32`, and `run.log` at `90+87`.

### Search operations

1. Core pack plus token and 3-gram sidecar rebuilds
2. Token query with `--max-results 1`
3. 3-gram query with candidate/decode/result budgets
4. Too-short `IN` query against the 3-gram sidecar

```text
bounded token: capped=true, candidate_granules=2, decoded_bytes=104,
               physical_decoded_bytes=452, verified_matches=1
tight n-gram:  capped=true, decoded_bytes=0, physical_decoded_bytes=0
short query:  incomplete_reason=query_shorter_than_ngram_n
```

## Template validation

- The workflow file parsed as YAML with Ruby's standard YAML parser. The exact
  copies embedded in both artifact guides are guarded by tests.
- `actionlint` and `systemd-analyze` were not installed on the macOS validation
  host. Tests therefore freeze the workflow's required job/actions and the
  unit files' required sections, `ExecStart`, `OnFailure`, timer, and alert
  contract. Linux operators should additionally run `systemd-analyze verify`
  after installing the templates because local systemd versions and paths vary.
- Independent review subsequently passed `actionlint v1.7.7` and Debian stable
  `systemd-analyze verify`; its security review drove the non-root users,
  resource limits, sandboxing, pipe failure handling, and provenance fields.

The test suite does not assert volatile timing fields. It checks command,
schema, linkage, safety-boundary, and template contracts that must remain true.
