#!/bin/sh
set -eu

workflow=".github/workflows/release.yml"

harden_workflow() {
    hardened="$(mktemp "${TMPDIR:-/tmp}/qzt-hardened-workflow.XXXXXX")"
    awk \
        -v plan_fragment="scripts/release-plan-install.yml" \
        -v build_fragment="scripts/release-build-install.yml" \
        -v local_record_fragment="scripts/release-build-record-local.yml" \
        -v global_record_fragment="scripts/release-build-record-global.yml" \
        -v build_verify_fragment="scripts/release-build-verify.yml" \
        -f scripts/harden-release-workflow.awk \
        "$workflow" > "$hardened"
    mv "$hardened" "$workflow"

    # Fail closed if cargo-dist changes the generated structure such that the
    # hardening transform no longer recognizes every sensitive location.
    grep -Fq '  "contents": "read"' "$workflow"
    test "$(grep -Fc '      "contents": "write"' "$workflow")" -eq 1
    grep -Fq 'environment: release' "$workflow"
    grep -Fq 'Validate release tag and main ancestry' "$workflow"
    grep -Fq 'Install digest-pinned dist (Windows)' "$workflow"
    test "$(grep -Fc 'Record the toolchain used by the' "$workflow")" -eq 2
    test "$(grep -Fc 'Verify the dist build kept its selected toolchain' "$workflow")" -eq 2
    test "$(grep -Fc 'env.BUILD_ENV_NAME' "$workflow")" -eq 2
    grep -Fq 'rm -f artifacts/*-build-environment.json' "$workflow"
    test "$(grep -Ec '^[[:space:]]+dist .*--allow-dirty --output-format=json' "$workflow")" -eq 4
    if grep -Fq 'cargo-dist-installer.sh | sh' "$workflow" || \
        grep -Fq 'matrix.install_dist.run' "$workflow"; then
        echo "unsafe cargo-dist installer survived workflow hardening" >&2
        exit 1
    fi
}

if [ "${1:-}" = "--check" ]; then
    snapshot="$(mktemp "${TMPDIR:-/tmp}/qzt-release-workflow.XXXXXX")"
    cp "$workflow" "$snapshot"
    restore() {
        cp "$snapshot" "$workflow"
        rm -f "$snapshot"
    }
    trap restore EXIT HUP INT TERM

    dist generate
    harden_workflow
    if ! cmp -s "$snapshot" "$workflow"; then
        diff -u "$snapshot" "$workflow" || true
        echo "release workflow is stale; run scripts/generate-release-workflow.sh" >&2
        exit 1
    fi
else
    dist generate
    harden_workflow
fi
