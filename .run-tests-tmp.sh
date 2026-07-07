#!/bin/sh
set -e
cd /Volumes/Satechi/Developer/qzt
echo "=== COMMAND 1 ==="
cargo test --test phase9_cli_core stdin -- --nocapture
echo "EXIT_CODE_1=$?"
echo "=== COMMAND 2 ==="
make check
echo "EXIT_CODE_2=$?"
