# Repository verification commands

| Scope | Command | Where it runs |
| --- | --- | --- |
| Focused CLI file output | `cargo test --test phase44_safe_file_output -- --nocapture` | local macOS; CI Linux, macOS, Windows |
| File output fault injection | `cargo test --bin qzt atomic_output_tests` | local macOS; CI Linux, macOS, Windows |
| Full local gate | `make check` | local and CI Linux |
| Documentation gate | `make doc` | local and CI Linux |
| Package gate | `cargo package --allow-dirty` | local and CI Linux |
| Preview distribution contract | `cargo test --locked --test phase42_release_readiness --test phase43_distribution` | local and CI Linux |
| Candidate/published verifier and release manifest clean-check regressions | `python3 -m unittest discover -s scripts -p 'test_*.py'` | local and CI Linux (Python 3.12+) |
| Context-enabled extracted binary workflow | `python3 scripts/verify-public-workflow.py --binary <absolute-path> --binary-sha256 <expected-sha256> --expected-tag <explicit-tag> --target <native-target> --vectors-dir tests/vectors --output <fresh-report>` | explicitly selected native binary; see `docs/guides/public-workflow-smoke.md`; published pre.5 is not context-enabled |
| Candidate publication boundary regressions | `python3 -m unittest discover -s scripts -p 'test_release_workflow.py'` | local and CI Linux; temporary local Git remotes only |
| Candidate/release workflow syntax | `actionlint -shellcheck= .github/workflows/release-candidate.yml .github/workflows/release.yml .github/workflows/verify-published-release.yml .github/workflows/ci.yml` | local before candidate PR; generated release shell blocks retain pre-existing ShellCheck warnings |
| Pre.6 candidate and release build rehearsal | `.github/workflows/release-candidate.yml` PR run, then manual dispatch with the exact main merge SHA; it uses `scripts/release-workflow-build.sh` from the tag-only release workflow for native/global manifest generation, reading, and copying, then checks the assembled pre-host files; `scripts/record-build-environment.py` checks the selected build toolchain before and after `dist build` | native macOS ARM/Intel, Linux x64, Windows x64 and global/assembly Linux runners; read-only, 14-day CI artifacts; hosting and `release` environment not exercised |
| Published pre.5 assets and installers | `.github/workflows/verify-published-release.yml` on its PR; `scripts/verify-published-release.py` requires explicit tag, version, product SHA, release run ID/attempt, and verifier SHA while downloading actual Release URLs | native macOS ARM/Intel, Linux x64, Windows x64 runners; read-only; 14-day verification artifacts |
| QZI/DLI seed replay | `cargo test --manifest-path fuzz/Cargo.toml --test seed_replay --locked` | local and CI Linux fuzz job |
| Bounded ASan fuzz | `cargo +nightly fuzz run --sanitizer address <target> fuzz/corpus/<target> -- -max_total_time=60 -timeout=10 -max_len=256 -rss_limit_mb=1024 -malloc_limit_mb=128 -seed=294` | weekly/manual CI Linux; `<target>` is `qzi_search` or `dli_decode` |

`.github/workflows/ci.yml` runs on pushes to `main` and pull requests to
`main`. It uses Rust caching; there is no change-range selector. Security scans
run separately in `.github/workflows/security.yml` on PRs, main pushes,
schedule, and manual dispatch. Fuzz smoke is weekly and manual; it also runs
`open_verify` with a 4096-byte input cap. Tracked seeds are copied to the
ignored runtime corpus as described in `fuzz/README.md`. The platform
file-output jobs exercise filesystem-specific behavior. Linux's targeted job
installs `acl` so its ACL regression test runs; without that tool, the test
reports a skip.

The pre.6 candidate workflow is separate from the protected, tag-only
`.github/workflows/release.yml`. It runs on changes to its own workflow file
or related release build files in a PR, or by manual dispatch with a full SHA
in main history. It has read-only repository permission, uses the same manifest
build/read/copy script as the release workflow without hosting or publishing,
and requires public-workflow-v1 on each extracted native archive. Its F1 gate
checks fixed structures, Reader Core, QZI v1/v2 and frozen vectors. A separate
native Windows job runs the unmodified PowerShell installer with explicit
loopback candidate staging and repeats the same C4 profile.
Run it again for the exact merge SHA; PR artifacts are not final-candidate
evidence. Its artifacts expire after 14 days.
After publication, PR runs report a skip and do not build candidate artifacts.
Manual candidate dispatch still rejects an existing tag, and a failed tag lookup
fails the plan. Use the separate read-only published verifier for published assets;
the candidate workflow cannot serve as post-publication evidence.
