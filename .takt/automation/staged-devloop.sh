#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
STATE_FILE="${TAKT_LOOP_STAGE_STATE:-${ROOT}/.takt/staged-devloop-state.env}"
TICK_SECONDS="${TAKT_LOOP_TICK_SECONDS:-60}"

ISSUE_SCOUT_INTERVAL="${TAKT_LOOP_ISSUE_SCOUT_INTERVAL:-14400}"
ISSUE_TO_PR_INTERVAL="${TAKT_LOOP_ISSUE_TO_PR_INTERVAL:-1800}"
PR_REVIEW_INTERVAL="${TAKT_LOOP_PR_REVIEW_INTERVAL:-1800}"
REVIEW_FIX_INTERVAL="${TAKT_LOOP_REVIEW_FIX_INTERVAL:-3600}"
PR_MERGE_INTERVAL="${TAKT_LOOP_PR_MERGE_INTERVAL:-1800}"

log() {
  printf '[%s] %s\n' "$(date '+%Y-%m-%dT%H:%M:%S%z')" "$*" >&2
}

now() {
  date '+%s'
}

load_state() {
  LAST_ISSUE_SCOUT=0
  LAST_ISSUE_TO_PR=0
  LAST_PR_REVIEW=0
  LAST_REVIEW_FIX=0
  LAST_PR_MERGE=0

  if [[ -f "$STATE_FILE" ]]; then
    # The state file is written by save_state below as simple timestamp assignments.
    # shellcheck disable=SC1090
    source "$STATE_FILE"
  fi
}

save_state() {
  mkdir -p "$(dirname "$STATE_FILE")"
  {
    printf 'LAST_ISSUE_SCOUT=%s\n' "$LAST_ISSUE_SCOUT"
    printf 'LAST_ISSUE_TO_PR=%s\n' "$LAST_ISSUE_TO_PR"
    printf 'LAST_PR_REVIEW=%s\n' "$LAST_PR_REVIEW"
    printf 'LAST_REVIEW_FIX=%s\n' "$LAST_REVIEW_FIX"
    printf 'LAST_PR_MERGE=%s\n' "$LAST_PR_MERGE"
  } >"$STATE_FILE"
}

due() {
  local last="$1"
  local interval="$2"
  local current
  current="$(now)"
  (( current - last >= interval ))
}

run_issue_scout() {
  log "stage issue-scout: creating safe product issues when backlog is empty"
  (cd "$ROOT" && ./.takt/automation/create-product-issues.sh create) || true
  LAST_ISSUE_SCOUT="$(now)"
}

run_issue_to_pr() {
  log "stage issue-to-pr: selecting one safe issue and creating/updating PR"
  (
    cd "$ROOT"
    TAKT_LOOP_AUTO_MERGE=0 \
    TAKT_LOOP_PR_REVIEW=0 \
      ./.takt/automation/full-auto-devloop.sh once
  ) || true
  LAST_ISSUE_TO_PR="$(now)"
}

run_pr_review() {
  log "stage pr-review: posting mergeability comments for green automation PRs"
  (cd "$ROOT" && ./.takt/automation/full-auto-devloop.sh review-open) || true
  LAST_PR_REVIEW="$(now)"
}

run_review_fix() {
  log "stage review-fix: fixing current-head Mergeable:NO automation PRs"
  (cd "$ROOT" && ./.takt/automation/full-auto-devloop.sh fix-open) || true
  LAST_REVIEW_FIX="$(now)"
}

run_pr_merge() {
  log "stage pr-merge: merging automation PRs that pass guards and review"
  (cd "$ROOT" && ./.takt/automation/full-auto-devloop.sh merge-open) || true
  LAST_PR_MERGE="$(now)"
}

run_once() {
  load_state

  if due "$LAST_ISSUE_SCOUT" "$ISSUE_SCOUT_INTERVAL"; then
    run_issue_scout
  fi
  if due "$LAST_ISSUE_TO_PR" "$ISSUE_TO_PR_INTERVAL"; then
    run_issue_to_pr
  fi
  if due "$LAST_PR_REVIEW" "$PR_REVIEW_INTERVAL"; then
    run_pr_review
  fi
  if due "$LAST_REVIEW_FIX" "$REVIEW_FIX_INTERVAL"; then
    run_review_fix
  fi
  if due "$LAST_PR_MERGE" "$PR_MERGE_INTERVAL"; then
    run_pr_merge
  fi

  save_state
}

case "${1:-once}" in
  once)
    run_once
    ;;
  loop)
    while true; do
      run_once
      log "stage scheduler sleeping ${TICK_SECONDS}s"
      sleep "$TICK_SECONDS"
    done
    ;;
  issue-scout)
    load_state
    run_issue_scout
    save_state
    ;;
  issue-to-pr)
    load_state
    run_issue_to_pr
    save_state
    ;;
  pr-review)
    load_state
    run_pr_review
    save_state
    ;;
  review-fix)
    load_state
    run_review_fix
    save_state
    ;;
  pr-merge)
    load_state
    run_pr_merge
    save_state
    ;;
  *)
    echo "usage: $0 [once|loop|issue-scout|issue-to-pr|pr-review|review-fix|pr-merge]" >&2
    exit 2
    ;;
esac
