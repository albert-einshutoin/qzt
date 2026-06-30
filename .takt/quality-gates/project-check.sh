#!/usr/bin/env bash
set -euo pipefail

MODE="${TAKT_LOOP_GATE_MODE:-standard}"

case "$MODE" in
  standard|full) ;;
  *)
    printf 'unsupported TAKT_LOOP_GATE_MODE: %s\n' "$MODE" >&2
    exit 2
    ;;
esac

run() {
  printf '\n==> %s\n' "$*"
  "$@"
}

has_command() {
  command -v "$1" >/dev/null 2>&1
}

check_git_whitespace() {
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    run git diff --check
  fi
}

check_go() {
  [[ -f go.mod ]] || return 0
  has_command go || {
    printf 'go.mod exists but go is not on PATH\n' >&2
    return 1
  }

  # Keep formatting read-only so the gate fails without rewriting agent output.
  local output
  output="$(find . \
    \( -path './.git' -o -path './.takt' -o -path './node_modules' -o -path './vendor' \) -prune \
    -o -name '*.go' -print0 | xargs -0 gofmt -l)"
  if [[ -n "$output" ]]; then
    printf 'gofmt required:\n%s\n' "$output" >&2
    return 1
  fi

  run go test ./...
  run go vet ./...

  if [[ "$MODE" == "full" ]]; then
    run go test -race ./...
    if has_command govulncheck; then
      run govulncheck ./...
    fi
  fi
}

node_package_manager() {
  if [[ -f pnpm-lock.yaml ]] && has_command pnpm; then
    printf 'pnpm'
  elif [[ -f yarn.lock ]] && has_command yarn; then
    printf 'yarn'
  else
    printf 'npm'
  fi
}

has_node_script() {
  local script="$1"
  [[ -f package.json ]] || return 1
  has_command node || return 1
  node -e "const p=require('./package.json'); process.exit(p.scripts && p.scripts[process.argv[1]] ? 0 : 1)" "$script"
}

run_node_script() {
  local pm="$1"
  local script="$2"
  case "$pm" in
    pnpm) run pnpm run "$script" ;;
    yarn) run yarn "$script" ;;
    npm) run npm run "$script" ;;
  esac
}

check_node() {
  [[ -f package.json ]] || return 0
  local pm
  pm="$(node_package_manager)"

  if has_node_script lint; then
    run_node_script "$pm" lint
  fi
  if has_node_script test; then
    run_node_script "$pm" test
  fi
  if has_node_script build; then
    run_node_script "$pm" build
  fi
  if [[ "$MODE" == "full" ]] && has_node_script audit; then
    run_node_script "$pm" audit
  fi
}

check_rust() {
  [[ -f Cargo.toml ]] || return 0
  has_command cargo || {
    printf 'Cargo.toml exists but cargo is not on PATH\n' >&2
    return 1
  }

  if has_command rustfmt; then
    run cargo fmt --all -- --check
  fi
  run cargo test --workspace --all-targets --all-features
  if [[ "$MODE" == "full" ]]; then
    run cargo clippy --workspace --all-targets --all-features -- -D warnings
  fi
}

check_python() {
  [[ -f pyproject.toml || -f requirements.txt || -d tests ]] || return 0

  if has_command ruff; then
    run ruff check .
  fi
  if has_command pytest; then
    # pytest exits 5 when no tests are collected; treat that as a pass for
    # projects (e.g. pure-Rust repos) that legitimately have no Python tests.
    printf '\n==> %s\n' pytest
    if ! pytest "$@"; then
      local rc=$?
      if [[ $rc -ne 5 ]]; then
        return $rc
      fi
    fi
  fi
}

check_git_whitespace
check_go
check_node
check_rust
check_python

