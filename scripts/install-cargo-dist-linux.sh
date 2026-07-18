#!/bin/sh
set -eu

archive="cargo-dist-x86_64-unknown-linux-gnu.tar.xz"
expected="cd355dab0b4c02fb59038fef87655550021d07f45f1d82f947a34ef98560abb8"
base="https://github.com/axodotdev/cargo-dist/releases/download/v0.31.0"
destination="${RUNNER_TEMP:-${TMPDIR:-/tmp}}"
path="${destination}/${archive}"

curl --proto '=https' --tlsv1.2 -fsSL "${base}/${archive}" -o "$path"
actual="$(shasum -a 256 "$path" | awk '{ print $1 }')"
test "$expected" = "$actual"
tar -xJf "$path" -C "$destination"
install -m 0755 "$destination/${archive%.tar.xz}/dist" "${HOME}/.cargo/bin/dist"
