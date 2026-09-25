#!/bin/sh
# Run the published pre.2 tour against the exact binary supplied by the caller.
set -eu

if [ "$#" -ne 1 ]; then
  echo "usage: $0 /absolute/path/to/release/qzt" >&2
  exit 2
fi
binary=$1
case "$binary" in
  /*) ;;
  *) echo "qzt binary path must be absolute" >&2; exit 2 ;;
esac
if [ ! -x "$binary" ]; then
  echo "qzt binary is missing or not executable: $binary" >&2
  exit 2
fi
command -v jq >/dev/null || { echo "jq is required" >&2; exit 2; }
command -v cmp >/dev/null || { echo "cmp is required" >&2; exit 2; }
test "$("$binary" --version)" = 'qzt 0.1.0-pre.2' || {
  echo "expected the published qzt 0.1.0-pre.2 binary" >&2
  exit 2
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"
printf 'alpha\nbeta\nerror gamma\n' > app.log
"$binary" pack app.log -o app.qzt
"$binary" info app.qzt --format json > info.json
jq -e '.format == "qzt-0.1" and .original_size == 23 and
  .line_count == 3 and .chunk_count == 1' info.json >/dev/null
"$binary" range app.qzt --lines 2:2 > range.out
printf 'beta\n' > expected-range.out
cmp expected-range.out range.out
"$binary" sidecar-rebuild app.qzt -o app.qzt.qzi
"$binary" search app.qzt error --sidecar app.qzt.qzi --format json > search.json
jq -e '.capped == false and .incomplete_reason == null and
  (.hits | length) == 1 and .hits[0].logical_offset == 11 and
  .hits[0].byte_length == 5 and
  .hits[0].source == "verified_original_bytes" and
  .metrics.verified_matches == 1' search.json >/dev/null
"$binary" verify app.qzt --deep --format json > verify.json
jq -e '.ok == true and .level == "deep" and
  .checked_chunks == 1 and .decoded_bytes == 23' verify.json >/dev/null
"$binary" attest app.qzt > attest.json
"$binary" attest app.qzt > attest-again.json
cmp attest.json attest-again.json
jq -e '.format == "qzt-0.1" and .original_size == 23 and
  .line_count == 3 and .verify.level == "deep" and
  .verify.checked_chunks == 1 and .verify.decoded_bytes == 23 and
  (has("attestation_schema") | not)' attest.json >/dev/null
"$binary" export app.qzt -o restored.log
cmp app.log restored.log

printf 'binary=%s\nversion=%s\nos=%s\narch=%s\n' \
  "$binary" "$("$binary" --version)" "$(uname -s)" "$(uname -m)"
printf 'pack/info/range/sidecar/search/deep verify/attest/export: PASS\n'
printf 'original and restored bytes: 23, cmp: PASS\n'
