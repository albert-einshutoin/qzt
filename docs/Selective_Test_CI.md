# Selective Test CI

QZT reduces pull-request test time without treating fewer tests as the goal.
The invariant is: select tests only when the changed surface can be bounded;
otherwise run the complete suite.

Japanese: [Selective_Test_CI.ja.md](Selective_Test_CI.ja.md)

## CI lanes

| Event | Test strategy | Other gates |
| --- | --- | --- |
| Pull request to `main` | Impact-selected, or full on uncertainty | Format, Clippy, all-target compilation, docs, package, dependency policy, Windows release build, security scans |
| Push to `main` or `release/**` | Full | Full coverage and the other gates |
| Daily schedule | Full | Full coverage and security scans |
| Manual dispatch | Full | Full coverage and the other gates |

Coverage is intentionally excluded from pull requests because it executes the
whole suite again. It remains a full post-merge, release, and daily signal.
The stable toolchain runs selected tests; the stable and MSRV lanes both
compile all targets, so language-version compatibility is never inferred from
the selection result.

## Architecture

The implementation separates policy from tool-specific execution:

```text
ci/impact.py
  change detection -> risk rules -> dependency graph -> test plan -> runner
        |                    |                              |
ci/config/impact.json        |                    target/ci/*.json
                             |
              ci/adapters/rust.json + rust.py
```

`ci/impact.py` is the CI-service-neutral common layer. It accepts explicit base
and head revisions, reads NUL-delimited Git rename/copy/delete records, emits a
JSON plan, and executes commands as argument arrays without a shell.

`ci/config/impact.json` owns repository policy:

- full-suite path rules;
- safe non-code paths;
- smoke targets;
- E2E classification;
- manual module-to-test and path-to-test mappings.

`ci/adapters/rust.json` owns Cargo-specific project detection, static/build and
test commands, cache locations, source/test globs, and additional dangerous
files. `ci/adapters/rust.py` owns Rust module, import, and public re-export
analysis. Another language or build tool should be added as another adapter;
its dependency parser and commands do not belong in the common planner.

## Impact decision

For a pull request, the planner:

1. verifies that both revisions exist;
2. reads `git diff --name-status -z -M -C BASE...HEAD`;
3. applies full-suite rules before narrowing anything;
4. detects the Cargo project and classifies every changed path;
5. builds a Rust module graph from `crate::module` dependencies;
6. walks reverse dependents from each changed module;
7. finds tests using qualified modules and public re-exports;
8. adds manual module/path mappings and directly changed tests;
9. adds the permanent smoke target;
10. verifies that every requested manual target still exists.

The QZT runner executes selected targets sequentially. Cargo shares one build
directory, so creating several hosted jobs would repeat compilation and can
increase total billed compute even when wall time decreases. The JSON target
lists remain suitable for a bounded matrix if measurements later show that a
different repository benefits from parallel execution.

## Full-suite fallback

Full validation is selected for dependency manifests and locks, CI/build/test
configuration, shared format/schema/error/I/O/resource-limit code, fuzz
configuration, missing revisions, malformed Git output, unsupported paths,
dependency-graph errors, invalid impact configuration, and stale manual test
mappings. A malformed adapter is a hard CI failure because no trustworthy full
command can be recovered from it.

Deleted paths remain evidence in `deletedFiles`, but only existing test files
become runner targets. A failed selector or runner never converts into a
successful skipped-test job.

## Evidence and local use

The workflow stores the plan and execution summary for 14 days. They include
revisions, changed/deleted files, modules, test categories, fallback reason,
target-level success/failure/skip counts, and duration.

```sh
python3 ci/impact.py plan \
  --repository . \
  --base origin/main \
  --head HEAD \
  --config ci/config/impact.json \
  --adapter ci/adapters/rust.json \
  --output target/ci/impact-plan.json

python3 ci/impact.py run \
  --repository . \
  --plan target/ci/impact-plan.json \
  --adapter ci/adapters/rust.json \
  --summary target/ci/test-summary.json
```

Run `make ci-test` after changing the planner, policy, adapter, or workflow.
Add a manual mapping when a test reaches behavior through a CLI, generated
surface, documentation contract, or another relationship that static Rust
imports cannot express.
