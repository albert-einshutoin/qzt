#!/usr/bin/env bash
set -euo pipefail

manifest=target/release-workflow/dist-manifest.json
action=${1:?expected build, print-local, print-global, or copy}
shift

case "$action" in
  build)
    mkdir -p "$(dirname "$manifest")"
    dist build "$@" > "$manifest"
    ;;
  print-local)
    test "$#" -eq 0
    dist print-upload-files-from-manifest --manifest "$manifest"
    ;;
  print-global)
    test "$#" -eq 0
    jq --raw-output '.upload_files[]' "$manifest"
    ;;
  copy)
    test "$#" -eq 0
    cp "$manifest" "${BUILD_MANIFEST_NAME:?BUILD_MANIFEST_NAME is required}"
    ;;
  *)
    echo "unknown release build action: $action" >&2
    exit 2
    ;;
esac
