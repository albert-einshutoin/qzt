# Security CI Selection Guide

Date: 2026-06-16

This guide is a reusable baseline for adding lightweight security checks to QZT
and other repositories. Treat the workflow as a starting point: enable broad
coverage first, review the first baseline, then tighten the failure policy.

## Tool Roles and Selection Criteria

| Tool | Primary role | Use when | Selection criteria | Overlap and cautions |
| --- | --- | --- | --- | --- |
| Semgrep CE (`semgrep/semgrep`) | Source-level SAST and secure coding rules | The repository has supported source languages and you want PR-time checks for insecure APIs, bug patterns, or project coding rules | Start with a language ruleset such as `p/rust` for a focused repo, or `p/ci` for a mixed-language repo. Pin the container image and update it deliberately. | It can overlap with secret and dependency products in the Semgrep platform, but this baseline uses CE as SAST only. Review false positives before making every severity blocking. |
| Gitleaks CLI / `gitleaks/gitleaks-action` | Secret scanning in the working tree and Git history | Almost every repo, especially public OSS, infra, examples, tests, fixtures, and config-heavy projects | Use `gitleaks-action` in GitHub Actions and the CLI or pre-commit locally. Use `fetch-depth: 0` when history scanning matters. | It does not replace SAST or dependency scanning. Organization-owned repositories may need `GITLEAKS_LICENSE`; personal repositories do not. |
| OSV Scanner / `google/osv-scanner-action` | Dependency vulnerability scanning from lockfiles, manifests, SBOMs, and optionally images | The repository has lockfiles or package manifests such as `Cargo.lock`, `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`, `go.sum`, `poetry.lock`, or Maven/Gradle files | Use as the default cross-ecosystem SCA gate. Prefer explicit lockfile arguments for small repos and recursive scans for monorepos. Upload SARIF when GitHub Code Scanning is available. | It overlaps with CVE Lite for JS/TS dependency findings. Keep OSV as the broad scanner; add CVE Lite only when its JS/TS remediation workflow is valuable. |
| OWASP CVE Lite CLI | JS/TS-focused dependency vulnerability scanning with remediation guidance | npm, pnpm, Yarn, or Bun projects need developer-friendly fix commands, direct/transitive visibility, SARIF, JSON, SBOM, or offline advisory DB support | Add it to JS/TS repos when the team wants `fail-on` severity thresholds and actionable remediation. Run locally before PRs if dependency churn is frequent. | Skip it for Rust-only, Go-only, or Python-only repositories. It intentionally duplicates some OSV dependency detection for JS/TS, but adds JS/TS-specific remediation ergonomics. |

## Recommended Policy Levels

| Level | Semgrep | Gitleaks | OSV Scanner | CVE Lite CLI |
| --- | --- | --- | --- | --- |
| Baseline | Run on PRs and schedules, collect findings, tune ignores | Block obvious new leaks, create allowlists for test fixtures only | Report findings; consider non-blocking until the first dependency baseline is reviewed | Run on JS/TS repos with `fail-on: critical` or non-blocking SARIF |
| Standard | Fail on selected rulesets after suppressions are reviewed | Required check, full history scan on push and schedule | `fail-on-vuln: true` for lockfiles committed to the repo | `fail-on: high` for JS/TS repos |
| Strict | Add custom project rules and narrow suppressions | Add pre-commit or local CLI checks | Add image/SBOM scans and explicit ignore expiry policy | Add `--usage`, `--only-used`, offline DB, SARIF, and SBOM outputs where useful |

## Decision Rules

- Use Gitleaks in every repository unless another mandatory secret scanner
  already covers Git history and pull requests.
- Use Semgrep when the repository language is supported and the team can review
  initial false positives.
- Use OSV Scanner whenever the repository commits lockfiles or dependency
  manifests.
- Add CVE Lite CLI only to JavaScript/TypeScript repositories that use npm,
  pnpm, Yarn, or Bun lockfiles.
- For Rust-only repositories like QZT, use Semgrep, Gitleaks, and OSV Scanner;
  omit CVE Lite CLI.

## Reusable GitHub Actions Sample

Use this as a generic starting point for GitHub repositories. For a focused Rust
repository, set Semgrep to `p/rust`, set OSV to `--lockfile=Cargo.lock`, and
remove the `cve-lite-js` job.

```yaml
name: security

on:
  pull_request:
  push:
  workflow_dispatch:
  schedule:
    - cron: "17 18 * * *"

permissions:
  contents: read

jobs:
  semgrep:
    name: semgrep
    runs-on: ubuntu-latest
    container:
      image: semgrep/semgrep:1.166.0
    steps:
      - uses: actions/checkout@v6
      - name: Run Semgrep CE
        env:
          SEMGREP_SEND_METRICS: "off"
        run: semgrep scan --config p/ci --error --metrics=off .

  osv:
    name: osv-scanner
    uses: google/osv-scanner-action/.github/workflows/osv-scanner-reusable.yml@v2.3.8
    permissions:
      actions: read
      contents: read
      security-events: write
    with:
      scan-args: |-
        -r .
      fail-on-vuln: true
      upload-sarif: true

  gitleaks:
    name: gitleaks
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0
      - uses: gitleaks/gitleaks-action@v3
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          # Required for organization-owned repositories, not personal accounts.
          GITLEAKS_LICENSE: ${{ secrets.GITLEAKS_LICENSE }}
          GITLEAKS_ENABLE_COMMENTS: "false"

  cve-lite-js:
    name: cve-lite-js
    runs-on: ubuntu-latest
    permissions:
      contents: read
      security-events: write
    steps:
      - uses: actions/checkout@v6
      - name: Detect JS/TS lockfiles
        id: js-lockfiles
        shell: bash
        run: |
          if find . \
            -path ./node_modules -prune -o \
            -path ./target -prune -o \
            \( -name package-lock.json -o -name pnpm-lock.yaml -o -name yarn.lock -o -name bun.lock \) \
            -print -quit | grep -q .; then
            echo "present=true" >> "$GITHUB_OUTPUT"
          else
            echo "present=false" >> "$GITHUB_OUTPUT"
          fi
      - name: Run CVE Lite CLI
        if: steps.js-lockfiles.outputs.present == 'true'
        uses: OWASP/cve-lite-cli@v1
        with:
          path: "."
          verbose: "true"
          fail-on: high
          sarif: "true"
      - name: Upload CVE Lite SARIF
        if: always() && steps.js-lockfiles.outputs.present == 'true'
        uses: github/codeql-action/upload-sarif@v4
        with:
          sarif_file: ${{ github.workspace }}
```

## QZT Rust Specialization

```yaml
semgrep:
  run: semgrep scan --config p/rust --error --metrics=off .

osv:
  with:
    scan-args: |-
      --lockfile=Cargo.lock
    fail-on-vuln: true

# Omit cve-lite-js unless JS/TS lockfiles are added.
```

## Operational Cautions

- Pin action and container versions; refresh them intentionally.
- Review the first scan baseline before enabling strict blocking rules.
- Prefer narrow allowlists and ignore files with comments explaining why each
  suppression is acceptable.
- Keep SARIF uploads optional if fork pull requests or repository permissions
  cannot write to GitHub Code Scanning.
- Do not use `pull_request_target` for untrusted code scans unless the workflow
  is explicitly hardened.

## References

- Semgrep: <https://github.com/semgrep/semgrep>
- Gitleaks: <https://github.com/gitleaks/gitleaks>
- Gitleaks Action: <https://github.com/gitleaks/gitleaks-action>
- OSV Scanner: <https://github.com/google/osv-scanner>
- OSV Scanner Action: <https://github.com/google/osv-scanner-action>
- OWASP CVE Lite CLI: <https://github.com/OWASP/cve-lite-cli>
