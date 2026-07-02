#!/bin/bash
cd /Volumes/Satechi/Developer/qzt
cargo test --test phase13_sidecar corrupted_sidecar_cli_exits_without_panic 2>&1
echo "EXIT_CODE=$?"
