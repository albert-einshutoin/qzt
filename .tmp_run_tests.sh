#!/usr/bin/env bash
set -uo pipefail
cd /Volumes/Satechi/Developer/qzt

echo "=== COMMAND 1 ==="
cargo test --test phase9_cli_core -- --nocapture 2>&1
echo "EXIT_CODE_1=$?"

echo ""
echo "=== COMMAND 2 ==="
cargo test --all-targets --all-features pack_stdin -- --nocapture 2>&1
echo "EXIT_CODE_2=$?"
