# Tutorial validation records

## Published pre.5 Release asset: measured 2026-09-27

The [published prerelease](https://github.com/albert-einshutoin/qzt/releases/tag/v0.1.0-pre.5)
was built from `3bc7d2561c58b59cd166fed75bfbd45f2f4c0ebe` by the ordinary
[tag-only run 36317632199 attempt 1](https://github.com/albert-einshutoin/qzt/actions/runs/36317632199).
Plan, four native builds, global, host, and announce succeeded. The separate
[public verifier run 36318204357](https://github.com/albert-einshutoin/qzt/actions/runs/36318204357)
used verifier commit `e1d6e01839a8b04800e2d03ea9fb5cc3fa0bbf8f` and succeeded
on global plus four native runners. It downloaded the actual public URLs,
checked all 14 GitHub size/SHA-256 metadata values, four archive sidecars,
the source sidecar, the five exact `sha256.sum` entries, and the host final
manifest. It matched all 426 files and executable bits in `source.tar.gz` to
the fixed product Git tree, fetched separately from the verifier checkout.
Internal build-environment and individual manifest JSON were absent from the
public 14 asset names.

| Native target | Public archive SHA-256 | Extracted and installed binary SHA-256 | Candidate comparison |
| --- | --- | --- | --- |
| macOS ARM64 | `3cbcab43f1c79ff11b756af292524715d1ca618c0445e4157e775876e454a9d1` | `989e6f21d80c5f14ee90b27aabe156760f0c6992bc6a5520aaa1400ba20b29be` | archive differs; binary matches |
| macOS Intel | `8834e648dae90f63a18b220c336dd25b9cdd0e068112d47149880d8890d86aba` | `d662e7c0f2210b75a3d51ad69414d98caa4a63d38c5b044dc2ecba4f12fa7ea9` | archive differs; binary matches |
| Linux x64 | `3f5c3d723e2e70480896b9a180980b694f7db4b22b397549b0c90df6cba96c70` | `726b382c24d9947b57106a432d6acbf74b8997bcba6d3212d9a08d5ac172e524` | archive differs; binary matches |
| Windows x64 | `3b642ca8a435f8a0a2b150f8878ad5aa76c81ac5c6c51c1d0f281d5e43bd82be` | `9dabdfbbdd82ea989158b8b90c9be2aab6895e1f058357b0968cfdb99c4a6b0f` | archive and binary differ; `.text` matches |

Each native job independently checked its archive sidecar before extraction,
reported `qzt 0.1.0-pre.5`, passed the fixed CLI smoke, and installed from a
digest-checked public installer into an isolated temporary directory without
PATH modification. The installed binary hash matched the independently
verified archive binary. Linux needed only `libc.so.6` and `libgcc_s.so.1`;
Windows readonly output rejection passed. The macOS ARM GitHub runner lacked
`sha256sum`, so its shell installer reported that its **own** SHA-256 check
was skipped. Release metadata digest, separately downloaded sidecar, and
installed binary hash checks still passed. They use the same GitHub Release
channel and are not independent signing. The skip is recorded separately.

All four archive hashes differ from [candidate #321](https://github.com/albert-einshutoin/qzt/issues/321)
because archive entry timestamps changed across builds. macOS and Linux
archive member contents, including binaries, matched. The Windows executable
had the same size and `.text` hash; 28 bytes differed only in four PE/CodeView
timestamps and the 16-byte RSDS PDB GUID. The five published build-environment
records match candidate Rust/Cargo/cargo-dist versions, selected toolchains,
profiles, features, flags, OS/architecture/platform, and build commands;
runner instance names and run timestamps differ. The actual release builds
used rustc/Cargo 1.98.0 on both macOS targets and 1.98.1 on Linux, Windows,
and global; cargo-dist was 0.31.0. The verifier runner's Rust version is a
separate fact, not a build toolchain claim.

The bounded fixture used **published pre.3 and pre.5 macOS ARM binaries** after
separate public archive checks. Both read/search each other's QZT and token/
n-gram QZI, passed Deep verify and exact export, preserved hard-error and cap
semantics, and emitted identical canonical `qzt-attestation-v1` bytes for the
same QZT and verification conditions. The [raw durable evidence](../releases/v0.1.0-pre.5-publication-evidence.json)
keeps asset hashes, five build environments, candidate comparisons, artifact
IDs/digests/expiry, and native result JSON after CI artifacts expire. This is
a bounded smoke and fixture, not a performance measurement, independent
signature, or universal compatibility proof.

## Published pre.3 Release asset: measured 2026-09-26

The [published prerelease](https://github.com/albert-einshutoin/qzt/releases/tag/v0.1.0-pre.3)
was built by [Release run 36237333455](https://github.com/albert-einshutoin/qzt/actions/runs/36237333455)
from product commit `017d4d19739800773ab6a54adf636ff5a43ec1fc`.
The read-only [verification workflow](../../.github/workflows/verify-published-release.yml)
records the actual public URL, SHA-256, native archive smoke, installer result,
and verifier commit separately for each of four targets in
[Issue #313](https://github.com/albert-einshutoin/qzt/issues/313).
The native macOS ARM public archive and installer also passed a separate local
smoke. [Published verification run 36238702884](https://github.com/albert-einshutoin/qzt/actions/runs/36238702884)
downloaded all 14 public assets and succeeded on all four native targets. Each
archive matched its published sidecar before extraction. Both extracted and
installer-placed binaries reported `qzt 0.1.0-pre.3`, passed the complete
candidate smoke, and had the same binary SHA-256 within each target.

| Native target | Published archive SHA-256 | Published binary SHA-256 | Compared with #311 candidate |
| --- | --- | --- | --- |
| macOS ARM64 | `0e51e0a1945b984500ae27a21e56930ba4b06a744a639feffa696307b1255b20` | `2e77275b033d498fdd61e97352fdfb06ac80aa95f3ea790f2b5727aa3512872b` | archive differs; binary matches |
| macOS Intel | `ba6c9043101aa54ec4039a0e83060300ef80ead6aebdecaf717dba45c1fc02bc` | `2d95bd920f920178a86b557a6596217e479e53ddac73ddf27be862fd03daefc1` | archive differs; binary matches |
| Linux x64 | `a82b18c86d13f31b7d1ec4545f88dd4328a180de67c2c53ea87f18eb62ebdbab` | `c9e61dc1cb55bcf047592125c1898b8d829318b7a3e52c42384286f4a272b1aa` | archive differs; binary matches |
| Windows x64 | `4c8152fbfd0468cb796e531ac2ad728fff6fa267e6b142c725acf136f894a945` | `dad7b757ae12c8c66459f2365a3662257b0a3f2c6dff8ce2246a6a539320f072` | archive and binary differ; published smoke passes |

The [run artifacts](https://github.com/albert-einshutoin/qzt/actions/runs/36238702884)
retain the installer URL, selected target, temporary installed path, runner,
toolchain, archive and installed binary hashes, and per-flow smoke outcomes.
Linux dynamically needs only `libc.so.6` and `libgcc_s.so.1`, not `libzstd`.
The published `sha256.sum` lists exactly the four archives and `source.tar.gz`;
the unpublished #311 candidate had a source-only aggregate checksum. These
are distinct builds. Release build toolchain versions were not recorded by
the release workflow; verifier runner toolchains do not establish them.

## Published pre.2 Release asset: measured 2026-09-25

This is an execution of the downloaded distribution binary, not a local build
with the same version string. The English and Japanese README tours have the
same command sequence; `docs/guides/examples/smoke-release-tour.sh` runs it with an explicit
binary path and no fallback to a checkout build.

- Release: [`v0.1.0-pre.2`](https://github.com/albert-einshutoin/qzt/releases/tag/v0.1.0-pre.2), tag commit `be70d1c7e16d9e006069dd6eb0eced113fe8b065`.
- Asset: [`qzt-aarch64-apple-darwin.tar.xz`](https://github.com/albert-einshutoin/qzt/releases/download/v0.1.0-pre.2/qzt-aarch64-apple-darwin.tar.xz), SHA-256 `079dbc111eb59051721d5bbff91e268be3f3020274197645f3ea003321ed28fe`, matched its `.sha256` asset before extraction.
- Executed binary: extracted `qzt-aarch64-apple-darwin/qzt`, SHA-256 `6d1a17487366b13013e8880a816397a3ca79e1bdf4fb45363b7c36a27a74e7fa`; `--version` returned `qzt 0.1.0-pre.2`.
- Host: macOS/Darwin 27.0.0, arm64; Zsh 5.9 for download and the baseline run, POSIX `sh` for the smoke; `jq 1.7.1-apple`, `shasum`, `cmp`.
- The original README sequence reached `inspect-sidecar` and failed with exit `2`, stderr `qzt: unknown command 'inspect-sidecar'`. This is an observed distribution-binary failure. The earlier Issue report was based on tag-source inspection.

After removing that unsupported command, the published binary ran
`pack → info → range → sidecar-rebuild → search → deep verify → attest → export`:

| Check | Observed result |
| --- | --- |
| `info` | `original_size=23`, `line_count=3`, `chunk_count=1`, `format=qzt-0.1` |
| `range --lines 2:2` | exact bytes `beta\n` |
| sidecar search for `error` | one hit at logical offset 11, length 5, `source=verified_original_bytes`, `capped=false` |
| `verify --deep --format json` | `ok=true`, `level=deep`, one checked chunk, 23 decoded bytes |
| `attest` twice | valid JSON, byte-identical across runs; no `attestation_schema`, `verify` has only `checked_chunks`, `decoded_bytes`, `level` |
| `export` | `cmp` matched all 23 bytes of the input |

Reproduce after downloading and checksum-verifying the matching asset:

```sh
sh docs/guides/examples/smoke-release-tour.sh /absolute/path/to/qzt-aarch64-apple-darwin/qzt
```

The script rejects a missing binary (exit `2`) and does not invoke `cargo` or
download anything. Linux x86_64, Intel macOS, and Windows asset checksums were
listed by the Release, but their binaries were **not executed** in this record.
Checksum availability is not a cross-platform tour result.

The README's optional `cargo install --git ... --tag v0.1.0-pre.2 --locked qzt`
also completed. That separately built binary reported `qzt 0.1.0-pre.2` and
passed the same smoke; its SHA-256 was
`20fc6a73e77153e42ac1ee1897e421c646b99f4fec0394eb608d6f7d44f7862b`.
This does **not** replace the distribution-binary result above. A direct
`cargo info qzt@0.1.0 --registry crates-io` query returned `could not find`;
the README therefore keeps crates.io installation conditional on publication.

## Pinned development guides: measured 2026-09-25

The README's `cargo install --git ... --rev ad709214f1e8ae18eff6e9f0b633345e40d1617b --locked qzt` command installed
the exact development source revision. The resulting binary reported
`qzt 0.1.0` and had SHA-256
`cae7d34adb8f9091613f957532aef3106276caaeeba26ab1d15cc3eff70f9205`.
This is a source build, not the public pre.2 asset. On the same macOS arm64
host, fresh-file runs checked the guide fixtures and their `jq -e` expressions:

- Log preservation: pack from stdin, Deep verify, `qzt-attestation-v1` and
  coverage policy, token sidecar search, and `INC-4242` byte range all passed.
- Artifact fixation: `pack-docs`, v1 attestation with
  `document_index_status=source_checked`, document listing/restoration with
  `cmp`, and attestation regeneration with `cmp` all passed.
- Search operations: token and n-gram sidecars, bounded search, the exact
  development-field `jq -e` projection, and short-query
  `incomplete_reason=query_shorter_than_ngram_n` passed. The bounded search
  returned one verified hit, `capped=true`,
  `stop_reason=max_search_results`, `index_complete_declared=true`,
  `index_coverage_verified=false`, and `physical_decoded_chunks=1`.
- Applying that projection to the public pre.2 JSON rejected its missing
  fields with exit `5`: `development search fields are missing`. A `null`
  projection is not accepted as verified coverage.

## Historical source-build record and development examples

The 2026-07-19 record below used a locally built binary from source commit
`0705c74c2a1e683e9b2a2e2a7c21935b0e11e990`, not a downloaded Release
asset. Later edits introduced `qzt-attestation-v1` and #292 verify fields into
the examples. They must not be read as measurements from that July binary.
The numeric fixtures below are now expectations exercised by
`phase46_tutorials` against the development source build; the current guide
contract is pinned to `ad709214f1e8ae18eff6e9f0b633345e40d1617b`.

### Historical environment

- Historical source commit: `0705c74c2a1e683e9b2a2e2a7c21935b0e11e990`
- Binary: locally built release-mode CLI, not a verified distribution asset
- Host: macOS 26.5 (25F71), arm64
- JSON processor: `jq 1.7.1-apple`
- Date: 2026-07-19 JST

### Development fixture journeys

The following commands and fixture values are checked by the current
`phase46_tutorials` test. They are not archived stdout from the July binary.
The intentionally incomplete two-scalar n-gram query exits `0` and warns on
stderr in the development build.

#### Log preservation

1. `qzt pack - -o daily.qzt < daily.log`
2. `qzt attest daily.qzt > daily.attest.json`
3. Deep-coverage `jq -e` policy check
4. `qzt verify daily.qzt --deep --format json`
5. Token `sidecar-rebuild`, JSON search for `INC-4242`, and `range` using the
   returned `logical_offset` plus `byte_length`

```text
verify: {"ok":true,"level":"deep","checked_chunks":1,"compressed_checksum_chunks":1,"decoded_chunks":1,"decoded_bytes":288,"original_checksum_verified":true,"container_checksum_status":"verified","dense_line_index_status":"absent","document_index_status":"absent"}
hit: logical_offset=200 byte_length=8 source=verified_original_bytes
metrics: decoded_bytes=88 physical_decoded_bytes=288 verified_matches=1
range stdout: INC-4242
```

#### Artifact fixation

1. `qzt pack-docs report.txt metrics.csv run.log -o run-1234.qzt`
2. `qzt attest run-1234.qzt > run-1234.attest.json`
3. `qzt docs run-1234.qzt --format json`
4. Verified `qzt doc`, byte-for-byte `cmp`, regenerated attestation, and empty
   `diff -u`

Observed document IDs and byte spans were `report.txt` at `0+58`,
`metrics.csv` at `58+32`, and `run.log` at `90+87`.

#### Search operations

1. Core pack plus token and 3-gram sidecar rebuilds
2. Token query with `--max-results 1`
3. 3-gram query with candidate/decode/result budgets
4. Too-short `IN` query against the 3-gram sidecar

```text
bounded token: capped=true, candidate_granules=2, decoded_bytes=105,
               physical_decoded_bytes=452, verified_matches=1
tight n-gram:  capped=true, decoded_bytes=0, physical_decoded_bytes=0
short query:  incomplete_reason=query_shorter_than_ngram_n
```

### Template validation

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
