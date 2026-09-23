# Repository verification commands

| Scope | Command | Where it runs |
| --- | --- | --- |
| Focused CLI file output | `cargo test --test phase44_safe_file_output -- --nocapture` | local macOS; CI Linux, macOS, Windows |
| File output fault injection | `cargo test --bin qzt atomic_output_tests` | local macOS; CI Linux, macOS, Windows |
| Full local gate | `make check` | local and CI Linux |
| Documentation gate | `make doc` | local and CI Linux |
| Package gate | `cargo package --allow-dirty` | local and CI Linux |

`.github/workflows/ci.yml` runs on pushes to `main` and pull requests to
`main`. It uses Rust caching; there is no change-range selector. Security scans
run separately in `.github/workflows/security.yml` on PRs, main pushes,
schedule, and manual dispatch. Fuzz smoke is weekly and manual. The platform
file-output jobs exercise filesystem-specific behavior. Linux's targeted job
installs `acl` so its ACL regression test runs; without that tool, the test
reports a skip.
