#!/bin/sh
# Smoke the published pre.3 binary supplied by absolute path; requires jq and cmp.
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
test -x "$binary" || { echo "qzt binary is missing or not executable: $binary" >&2; exit 2; }
command -v jq >/dev/null || { echo "jq is required" >&2; exit 2; }
command -v cmp >/dev/null || { echo "cmp is required" >&2; exit 2; }
test "$("$binary" --version)" = 'qzt 0.1.0-pre.3' || {
  echo "expected the published qzt 0.1.0-pre.3 binary" >&2
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
"$binary" inspect-sidecar app.qzt --sidecar app.qzt.qzi --format json > sidecar.json
jq -e '.index_type == "token" and .complete == true and
  .source_size_bytes == 23 and .granule_count == 3' sidecar.json >/dev/null
"$binary" search app.qzt error --sidecar app.qzt.qzi --format json > search.json
jq -e '.capped == false and .stop_reason == null and
  .index_complete_declared == true and .index_coverage_verified == false and
  (.hits | length) == 1 and .hits[0].logical_offset == 11 and
  .hits[0].byte_length == 5 and
  .hits[0].source == "verified_original_bytes" and
  .metrics.verified_matches == 1' search.json >/dev/null
"$binary" search app.qzt error --sidecar app.qzt.qzi --max-results 1 --format json > capped.json
jq -e '.capped == true and .stop_reason == "max_search_results" and
  (.hits | length) == 1' capped.json >/dev/null
if "$binary" search app.qzt error --sidecar app.qzt.qzi --max-posting-bytes 0 --format json > hard.json 2> hard.err; then
  echo "hard posting limit unexpectedly succeeded" >&2
  exit 1
fi
test ! -s hard.json
grep -qi 'resource limit' hard.err
"$binary" verify app.qzt --deep --format json > verify.json
jq -e '.ok == true and .level == "deep" and
  .checked_chunks == 1 and .compressed_checksum_chunks == 1 and
  .decoded_chunks == 1 and .decoded_bytes == 23 and
  .original_checksum_verified == true' verify.json >/dev/null
"$binary" attest app.qzt > attest.json
"$binary" attest app.qzt > attest-again.json
cmp attest.json attest-again.json
jq -e '.attestation_schema == "qzt-attestation-v1" and
  .format == "qzt-0.1" and .original_size == 23 and .line_count == 3 and
  .verify.level == "deep" and .verify.checked_chunks == 1 and
  .verify.decoded_bytes == 23 and .verify.original_checksum_verified == true' attest.json >/dev/null
"$binary" export app.qzt -o restored.log
cmp app.log restored.log

printf 'binary=%s\nversion=%s\nos=%s\narch=%s\n' \
  "$binary" "$("$binary" --version)" "$(uname -s)" "$(uname -m)"
printf 'pack/info/range/sidecar/search/deep verify/attest/export: PASS\n'
printf 'original and restored bytes: 23, cmp: PASS\n'
