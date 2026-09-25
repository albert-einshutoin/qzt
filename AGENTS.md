# Repository verification commands

| Scope | Command | Where it runs |
| --- | --- | --- |
| Focused CLI file output | `cargo test --test phase44_safe_file_output -- --nocapture` | local macOS; CI Linux, macOS, Windows |
| File output fault injection | `cargo test --bin qzt atomic_output_tests` | local macOS; CI Linux, macOS, Windows |
| Full local gate | `make check` | local and CI Linux |
| Documentation gate | `make doc` | local and CI Linux |
| Package gate | `cargo package --allow-dirty` | local and CI Linux |
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
