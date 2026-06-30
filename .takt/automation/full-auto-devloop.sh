#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TAKT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

default_repo() {
  gh repo view --json nameWithOwner --jq '.nameWithOwner' 2>/dev/null || true
}

REPO="${TAKT_LOOP_REPO:-${GITHUB_REPOSITORY:-$(default_repo)}}"
WORKFLOW="${TAKT_LOOP_WORKFLOW:-.takt/workflows/subscription-devloop.yaml}"
INTERVAL_SECONDS="${TAKT_LOOP_INTERVAL_SECONDS:-300}"
MAX_AUTO_MERGE_FILES="${TAKT_LOOP_MAX_AUTO_MERGE_FILES:-12}"
MAX_AUTO_MERGE_LINES="${TAKT_LOOP_MAX_AUTO_MERGE_LINES:-500}"
AUTO_MERGE="${TAKT_LOOP_AUTO_MERGE:-1}"
PR_REVIEW="${TAKT_LOOP_PR_REVIEW:-1}"
CREATE_ISSUES="${TAKT_LOOP_CREATE_ISSUES:-0}"
AGY_MODEL="${TAKT_LOOP_AGY_MODEL:-Gemini 3.5 Flash (High)}"
AGY_PRINT_TIMEOUT="${TAKT_LOOP_AGY_PRINT_TIMEOUT:-5m}"
CODEX_REVIEW_MODEL="${TAKT_LOOP_CODEX_REVIEW_MODEL:-gpt-5.5}"
CODEX_REVIEW_REASONING_EFFORT="${TAKT_LOOP_CODEX_REVIEW_REASONING_EFFORT:-xhigh}"
CODEX_REVIEW_MAX_DIFF_LINES="${TAKT_LOOP_CODEX_REVIEW_MAX_DIFF_LINES:-2000}"
CODEX_HUMAN_REVIEW="${TAKT_LOOP_CODEX_HUMAN_REVIEW:-1}"
CURSOR_MODEL="${TAKT_LOOP_CURSOR_MODEL:-composer-2.5}"
ISSUE_CRAFTER="${TAKT_LOOP_ISSUE_CRAFTER:-.takt/automation/create-product-issues.sh}"
MERGE_METHOD="${TAKT_LOOP_MERGE_METHOD:-squash}"
MAX_ACTIVE_RUNS="${TAKT_LOOP_MAX_ACTIVE_RUNS:-1}"
ACTIVE_RUN_STALE_AFTER_MINUTES="${TAKT_LOOP_ACTIVE_RUN_STALE_AFTER_MINUTES:-180}"
CLEAR_STALE_ACTIVE_RUNS="${TAKT_LOOP_CLEAR_STALE_ACTIVE_RUNS:-1}"

READY_LABEL="${TAKT_LOOP_READY_LABEL:-agent:ready}"
AUTO_MERGE_LABEL="${TAKT_LOOP_AUTO_MERGE_LABEL:-agent:auto-merge}"
BLOCKED_LABEL="${TAKT_LOOP_BLOCKED_LABEL:-agent:blocked}"
PR_REVIEW_MARKER="<!-- takt-loop-mergeability-review -->"
CODEX_REVIEW_MARKER="<!-- takt-loop-codex-human-review -->"
ISSUE_BLOCK_MARKER="<!-- takt-loop-issue-automation-block -->"
PR_BLOCK_MARKER="<!-- takt-loop-pr-automation-block -->"

if [[ -z "$REPO" ]]; then
  echo "TAKT_LOOP_REPO is required when gh cannot infer the repository" >&2
  exit 2
fi

log() {
  printf '[%s] %s\n' "$(date '+%Y-%m-%dT%H:%M:%S%z')" "$*" >&2
}

run_quality_gate_for_worktree() {
  local worktree="$1"
  local gate

  for gate in \
    "${worktree}/.takt/quality-gates/project-check.sh" \
    "${TAKT_ROOT}/quality-gates/project-check.sh"; do
    if [[ -x "$gate" ]]; then
      # Detached PR worktrees often omit .takt because downstream repos gitignore it.
      # Run the template gate from the source repo while keeping the PR worktree as cwd.
      (cd "$worktree" && "$gate")
      return
    fi
  done

  log "quality gate script not found for detached worktree"
  return 1
}

ensure_label() {
  local name="$1"
  local description="$2"
  local color="$3"
  if gh label list --repo "$REPO" --limit 200 --json name --jq '.[].name' | grep -Fxq "$name"; then
    return
  fi
  local create_output
  if create_output="$(gh label create "$name" --repo "$REPO" --description "$description" --color "$color" 2>&1)"; then
    return
  fi
  if grep -Fq "already exists" <<<"$create_output"; then
    gh label edit "$name" --repo "$REPO" --description "$description" --color "$color" >/dev/null 2>&1 || true
    return 0
  fi
  printf '%s\n' "$create_output" >&2
  return 1
}

ensure_labels() {
  ensure_label "$READY_LABEL" "Ready for local TAKT/devloopd automation" "5319e7"
  ensure_label "$AUTO_MERGE_LABEL" "Mechanical gates passed; allow guarded local auto-merge" "0e8a16"
  ensure_label "$BLOCKED_LABEL" "Automation paused; human update required before retry" "d93f0b"
}

issue_pr_reference_count() {
  local issue="$1"
  local state="$2"
  [[ "$issue" =~ ^[0-9]+$ ]] || return 1

  local jq_filter
  jq_filter='def pr_title: (.title // "");
    def pr_body: (.body // "");
    def pr_branch: (.headRefName // "");
    [ .[] | select(
      (pr_title | test("(^|[^0-9])#'"${issue}"'([^0-9]|$)")) or
      (pr_branch | test("(^|[^A-Za-z0-9])issue[-_/]'"${issue}"'([^A-Za-z0-9]|$)")) or
      (pr_branch | test("(^|[^A-Za-z0-9])daily-issue-implementation[-_/]'"${issue}"'([^A-Za-z0-9]|$)")) or
      (pr_body | test("(^|[[:space:][:punct:]])([Cc]loses|[Cc]lose|[Cc]losed|[Ff]ixes|[Ff]ix|[Ff]ixed|[Rr]esolves|[Rr]esolve|[Rr]esolved)[[:space:]]+#'"${issue}"'([^0-9]|$)"))
    ) ] | length'

  gh pr list --repo "$REPO" --state "$state" --limit 200 \
    --json number,title,body,headRefName \
    --jq "$jq_filter"
}

issue_has_existing_pr() {
  local issue="$1"
  [[ "$(issue_pr_reference_count "$issue" all)" != "0" ]]
}

candidate_from_scan() {
  devloopd scan-issues --repo "$REPO" \
    | awk '/^Candidates:/{inside=1; next} /^Skipped:/{inside=0} inside && /^- #/{sub(/^- #/, ""); sub(/ .*/, ""); print; exit}'
}

title_is_broad() {
  local title="$1"
  [[ "$title" =~ (トラッキング|全体計画|最大化計画|ロードマップ|roadmap|Roadmap|tracking|Tracking|strategy|Strategy) ]]
}

labels_are_forbidden() {
  local labels="$1"
  [[ ",${labels}," == *",blocked,"* \
    || ",${labels}," == *",${BLOCKED_LABEL},"* \
    || ",${labels}," == *",human-required,"* \
    || ",${labels}," == *",security-sensitive,"* \
    || ",${labels}," == *",do-not-touch,"* \
    || ",${labels}," == *",duplicate,"* \
    || ",${labels}," == *",invalid,"* \
    || ",${labels}," == *",wontfix,"* ]]
}

remove_ready_if_needed() {
  local issue="$1"
  gh issue edit "$issue" --repo "$REPO" --remove-label "$READY_LABEL" >/dev/null 2>&1 || true
}

issue_has_block_comment() {
  local issue="$1"
  gh api "repos/${REPO}/issues/${issue}/comments" --paginate --jq '.[].body // ""' \
    | grep -Fq "$ISSUE_BLOCK_MARKER"
}

block_issue_for_automation() {
  local issue="$1"
  local reason="$2"
  local details="$3"
  local tmpfile

  remove_ready_if_needed "$issue"
  gh issue edit "$issue" --repo "$REPO" --add-label "$BLOCKED_LABEL" >/dev/null 2>&1 || true

  if issue_has_block_comment "$issue"; then
    log "blocked issue #${issue} for automation: ${reason}"
    return 0
  fi

  tmpfile="$(mktemp)"
  cat >"$tmpfile" <<EOF
${ISSUE_BLOCK_MARKER}
Automation paused this issue and removed \`${READY_LABEL}\`.

Reason: ${reason}

Details:
${details}

To retry, update the issue or unblock the prerequisite, remove \`${BLOCKED_LABEL}\`, and add \`${READY_LABEL}\` again.
EOF
  gh issue comment "$issue" --repo "$REPO" --body-file "$tmpfile" >/dev/null 2>&1 || true
  rm -f "$tmpfile"
  log "blocked issue #${issue} for automation: ${reason}"
}

clear_stale_active_runs() {
  [[ "$CLEAR_STALE_ACTIVE_RUNS" == "1" ]] || return 0
  [[ -d ".takt/runs" ]] || return 0
  command -v node >/dev/null 2>&1 || {
    log "node not found; stale active run cleanup skipped"
    return 0
  }

  local report slug meta
  report="$(devloopd active-runs --cwd "$(pwd)" --stale-after-minutes "$ACTIVE_RUN_STALE_AFTER_MINUTES" 2>/dev/null || true)"
  while IFS= read -r slug; do
    [[ -n "$slug" ]] || continue
    meta=".takt/runs/${slug}/meta.json"
    [[ -f "$meta" ]] || continue
    # Stale running metadata blocks future issue work even though no worker is alive.
    # Mark only stale runs failed so active non-stale runs still preserve the concurrency guard.
    node - "$meta" <<'NODE'
const fs = require("node:fs");
const path = process.argv[2];
const meta = JSON.parse(fs.readFileSync(path, "utf8"));
if (meta.status === "running") {
  meta.status = "failed";
  meta.completedAt = new Date().toISOString();
  meta.failureReason = "Marked failed by full-auto-devloop stale active-run cleanup.";
  fs.writeFileSync(path, `${JSON.stringify(meta, null, 2)}\n`);
}
NODE
    log "marked stale TAKT run ${slug} as failed"
  done < <(printf '%s\n' "$report" | awk '/^- / && /\[stale\]/{sub(/^- /, ""); sub(/ .*/, ""); print}')
}

find_existing_safe_candidate() {
  local candidate seen_candidates=""
  while candidate="$(candidate_from_scan)" && [[ -n "$candidate" ]]; do
    if [[ " ${seen_candidates} " == *" ${candidate} "* ]]; then
      log "stopping candidate scan after repeated stale candidate #${candidate}"
      return 1
    fi
    seen_candidates="${seen_candidates} ${candidate}"
    if issue_has_existing_pr "$candidate"; then
      # A ready issue with an existing PR would make devloopd rerun the same work.
      block_issue_for_automation "$candidate" "Duplicate or already covered" "- An open or historical PR already references this issue."
      continue
    fi
    printf '%s\n' "$candidate"
    return 0
  done
  return 1
}

try_mark_issue_ready() {
  local issue="$1"
  gh issue edit "$issue" --repo "$REPO" --add-label "$READY_LABEL" >/dev/null

  local selected
  selected="$(candidate_from_scan || true)"
  if [[ "$selected" == "$issue" ]]; then
    printf '%s\n' "$issue"
    return 0
  fi

  remove_ready_if_needed "$issue"
  log "skipped #${issue}: devloopd did not classify it as an automation candidate"
  return 1
}

mark_next_issue_ready() {
  if find_existing_safe_candidate; then
    return 0
  fi

  local line issue title labels
  while IFS=$'\t' read -r issue title labels; do
    [[ -z "${issue:-}" ]] && continue
    if title_is_broad "$title"; then
      log "skipped #${issue}: broad tracker title"
      continue
    fi
    if labels_are_forbidden "$labels"; then
      log "skipped #${issue}: forbidden label"
      continue
    fi
    if issue_has_existing_pr "$issue"; then
      block_issue_for_automation "$issue" "Duplicate or already covered" "- An open or historical PR already references this issue."
      continue
    fi
    if try_mark_issue_ready "$issue"; then
      return 0
    fi
  done < <(
    gh issue list --repo "$REPO" --state open --limit 100 --json number,title,labels \
      --jq '.[] | select(([.labels[].name] | index("'"${READY_LABEL}"'")) | not) | "\(.number)\t\(.title)\t\([.labels[].name] | join(","))"'
  )

  return 1
}

find_open_pr_for_issue() {
  local issue="$1"
  [[ "$issue" =~ ^[0-9]+$ ]] || return 1

  local jq_filter
  jq_filter='def pr_title: (.title // "");
    def pr_body: (.body // "");
    def pr_branch: (.headRefName // "");
    [ .[] | select(
      (pr_title | test("(^|[^0-9])#'"${issue}"'([^0-9]|$)")) or
      (pr_branch | test("(^|[^A-Za-z0-9])issue[-_/]'"${issue}"'([^A-Za-z0-9]|$)")) or
      (pr_branch | test("(^|[^A-Za-z0-9])daily-issue-implementation[-_/]'"${issue}"'([^A-Za-z0-9]|$)")) or
      (pr_body | test("(^|[[:space:][:punct:]])([Cc]loses|[Cc]lose|[Cc]losed|[Ff]ixes|[Ff]ix|[Ff]ixed|[Rr]esolves|[Rr]esolve|[Rr]esolved)[[:space:]]+#'"${issue}"'([^0-9]|$)"))
    ) | .number ][0] // empty'

  gh pr list --repo "$REPO" --state open --limit 200 \
    --json number,title,body,headRefName \
    --jq "$jq_filter"
}

file_mtime() {
  stat -f '%m' "$1" 2>/dev/null || stat -c '%Y' "$1" 2>/dev/null || printf '0'
}

latest_issue_response_file() {
  local issue="$1"
  local file

  while IFS= read -r file; do
    printf '%s\t%s\n' "$(file_mtime "$file")" "$file"
  done < <(find .takt/runs -type f -path "*/issue-${issue}-*/context/previous_responses/latest.md" -print 2>/dev/null) \
    | sort -rn \
    | head -n 1 \
    | cut -f2-
}

classify_issue_without_pr() {
  local issue="$1"
  local start_log="$2"
  local response_file

  response_file="$(latest_issue_response_file "$issue")"
  if [[ -n "$response_file" ]]; then
    if grep -Fq "Duplicate or already covered" "$response_file"; then
      printf 'Duplicate or already covered'
      return
    fi
    if grep -Fq "Unsafe or too broad" "$response_file"; then
      printf 'Unsafe or too broad'
      return
    fi
    if grep -Fq "Cannot proceed" "$response_file"; then
      printf 'Cannot proceed'
      return
    fi
  fi

  if grep -Fq "active run limit reached" "$start_log"; then
    printf 'Active run limit reached'
    return
  fi

  printf 'No PR created'
}

block_issue_without_pr() {
  local issue="$1"
  local start_status="$2"
  local start_log="$3"
  local reason details response_file

  reason="$(classify_issue_without_pr "$issue" "$start_log")"
  if [[ "$reason" == "Active run limit reached" ]]; then
    log "active run limit reached for issue #${issue}; leaving ${READY_LABEL} for a later retry"
    return 0
  fi

  response_file="$(latest_issue_response_file "$issue")"

  details="- devloopd start exit code: ${start_status}"
  if [[ -n "$response_file" ]]; then
    details="${details}
- latest TAKT response: ${response_file}"
  fi
  if [[ -s "$start_log" ]]; then
    details="${details}
- start log tail:
\`\`\`
$(tail -40 "$start_log")
\`\`\`"
  fi

  block_issue_for_automation "$issue" "$reason" "$details"
}

path_is_forbidden_for_auto_merge() {
  local path="$1"
  case "$path" in
    .github/*|infra/*|terraform/*|migrations/*|db/migrations/*|auth/*|billing/*|payments/*|security/*|*.env*|*secret*|*credential*|*private-key*)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

pr_passes_local_merge_guard() {
  local pr="$1"
  local changed_files additions deletions total_lines path
  changed_files="$(gh pr view "$pr" --repo "$REPO" --json changedFiles --jq '.changedFiles')"
  additions="$(gh pr view "$pr" --repo "$REPO" --json additions --jq '.additions')"
  deletions="$(gh pr view "$pr" --repo "$REPO" --json deletions --jq '.deletions')"
  total_lines=$((additions + deletions))

  if (( changed_files > MAX_AUTO_MERGE_FILES )); then
    log "PR #${pr} requires human review: changed files ${changed_files} > ${MAX_AUTO_MERGE_FILES}"
    return 1
  fi
  if (( total_lines > MAX_AUTO_MERGE_LINES )); then
    log "PR #${pr} requires human review: changed lines ${total_lines} > ${MAX_AUTO_MERGE_LINES}"
    return 1
  fi

  while IFS= read -r path; do
    if path_is_forbidden_for_auto_merge "$path"; then
      log "PR #${pr} requires human review: forbidden auto-merge path ${path}"
      return 1
    fi
  done < <(gh pr diff "$pr" --repo "$REPO" --name-only)

  return 0
}

existing_mergeability_decision() {
  local pr="$1"
  local head_sha="$2"

  gh api "repos/${REPO}/issues/${pr}/comments" --paginate \
    --jq "map(select(((.body // \"\") | contains(\"${PR_REVIEW_MARKER}\")) and ((.body // \"\") | contains(\"Head SHA: \`${head_sha}\`\")))) | last | .body // \"\" | if test(\"(?m)^Mergeable:[[:space:]]*YES\") then \"YES\" elif test(\"(?m)^Mergeable:[[:space:]]*NO\") then \"NO\" else \"\" end"
}

comment_pr_mergeability_review() {
  local pr="$1"
  local gate_context="$2"
  [[ "$PR_REVIEW" == "1" ]] || return 0

  local head_sha existing
  head_sha="$(gh pr view "$pr" --repo "$REPO" --json headRefOid --jq '.headRefOid')"
  existing="$(existing_mergeability_decision "$pr" "$head_sha" || true)"
  if [[ "$existing" == "YES" ]]; then
    log "agy mergeability review already approved PR #${pr} at ${head_sha}"
    return 0
  fi
  if [[ "$existing" == "NO" ]]; then
    log "agy mergeability review already blocked PR #${pr} at ${head_sha}"
    return 1
  fi

  local tmpdir prompt_file review_file body_file
  tmpdir="$(mktemp -d)"
  prompt_file="${tmpdir}/prompt.md"
  review_file="${tmpdir}/review.md"
  body_file="${tmpdir}/body.md"

  local metadata files checks
  metadata="$(gh pr view "$pr" --repo "$REPO" --json number,title,url,baseRefName,headRefName,headRefOid,mergeable,mergeStateStatus,reviewDecision,isDraft,changedFiles,additions,deletions,labels)"
  files="$(gh pr diff "$pr" --repo "$REPO" --name-only || true)"
  checks="$(gh pr checks "$pr" --repo "$REPO" 2>&1 || true)"

  cat >"$prompt_file" <<EOF
You are the PR mergeability reviewer using agy ${AGY_MODEL}.

Use only the supplied GitHub metadata, check output, changed-file list, and
local automation policy. Do not ask for secrets, CI bypass, force pushes, or
admin merges. The TAKT workflow already ran code review before PR creation;
this pass decides whether the current PR head is mergeable.

Return exactly this shape:

Mergeable: YES|NO
Reason: one concise sentence
Blockers:
- none, or concrete blockers with file paths/commands
Verification:
- checks/local guard evidence used

Gate context:
${gate_context}

PR metadata:
${metadata}

Changed files:
${files}

GitHub checks:
${checks}
EOF

  if ! agy --model "$AGY_MODEL" --print-timeout "$AGY_PRINT_TIMEOUT" -p "$(cat "$prompt_file")" >"$review_file"; then
    cat >"$review_file" <<EOF
Mergeable: NO
Reason: agy mergeability review failed to run.
Blockers:
- Review command failed; leave the PR open for manual inspection.
Verification:
- Gate context: ${gate_context}
EOF
  fi

  if ! grep -Eq '^Mergeable:[[:space:]]*(YES|NO)[[:space:]]*$' "$review_file"; then
    {
      printf 'Mergeable: NO\n'
      printf 'Reason: agy review output did not follow the required contract.\n'
      printf 'Blockers:\n'
      printf -- '- Normalize the PR review result before merging.\n'
      printf 'Verification:\n'
      printf -- '- Gate context: %s\n\n' "$gate_context"
      cat "$review_file"
    } >"${tmpdir}/review.normalized.md"
    mv "${tmpdir}/review.normalized.md" "$review_file"
  fi

  {
    printf '%s\n' "$PR_REVIEW_MARKER"
    printf 'Head SHA: `%s`\n\n' "$head_sha"
    cat "$review_file"
  } >"$body_file"

  gh pr comment "$pr" --repo "$REPO" --body-file "$body_file" >/dev/null
  log "posted agy mergeability review for PR #${pr}"

  if grep -Eq '^Mergeable:[[:space:]]*YES[[:space:]]*$' "$review_file"; then
    rm -rf "$tmpdir"
    return 0
  fi

  rm -rf "$tmpdir"
  return 1
}

pr_has_block_comment() {
  local pr="$1"
  gh api "repos/${REPO}/issues/${pr}/comments" --paginate --jq '.[].body // ""' \
    | grep -Fq "$PR_BLOCK_MARKER"
}

block_pr_for_automation() {
  local pr="$1"
  local reason="$2"
  local details="$3"
  local tmpfile

  gh pr edit "$pr" --repo "$REPO" --remove-label "$AUTO_MERGE_LABEL" >/dev/null 2>&1 || true
  gh pr edit "$pr" --repo "$REPO" --add-label "$BLOCKED_LABEL" >/dev/null 2>&1 || true

  if pr_has_block_comment "$pr"; then
    log "blocked PR #${pr} for automation: ${reason}"
    return 0
  fi

  tmpfile="$(mktemp)"
  cat >"$tmpfile" <<EOF
${PR_BLOCK_MARKER}
Automation paused this PR and removed \`${AUTO_MERGE_LABEL}\`.

Reason: ${reason}

Details:
${details}

To retry automation, push a new fix if needed, remove \`${BLOCKED_LABEL}\`, and let the staged loop review the current head again.
EOF
  gh pr comment "$pr" --repo "$REPO" --body-file "$tmpfile" >/dev/null 2>&1 || true
  rm -f "$tmpfile"
  log "blocked PR #${pr} for automation: ${reason}"
}

comment_pr_mergeability_blocker() {
  local pr="$1"
  local reason="$2"
  local details="$3"
  local head_sha tmpfile

  head_sha="$(gh pr view "$pr" --repo "$REPO" --json headRefOid --jq '.headRefOid')"
  tmpfile="$(mktemp)"
  cat >"$tmpfile" <<EOF
${PR_REVIEW_MARKER}
Head SHA: \`${head_sha}\`

Mergeable: NO
Reason: ${reason}
Blockers:
${details}
Verification:
- Automated merge attempt failed after checks/review gates.
EOF
  gh pr comment "$pr" --repo "$REPO" --body-file "$tmpfile" >/dev/null 2>&1 || true
  rm -f "$tmpfile"
  log "posted mergeability blocker for PR #${pr}: ${reason}"
}

codex_human_review_approves() {
  local pr="$1"
  local head_sha="$2"
  local merge_log="$3"
  [[ "$CODEX_HUMAN_REVIEW" == "1" ]] || return 1
  command -v codex >/dev/null 2>&1 || return 1

  local tmpdir prompt_file review_file body_file codex_log diff_file metadata files checks diff_output diff_line_count mergeability_reviews
  tmpdir="$(mktemp -d)"
  prompt_file="${tmpdir}/prompt.md"
  review_file="${tmpdir}/review.md"
  body_file="${tmpdir}/body.md"
  codex_log="${tmpdir}/codex.log"
  diff_file="${tmpdir}/diff.patch"

  metadata="$(gh pr view "$pr" --repo "$REPO" --json number,title,url,baseRefName,headRefName,headRefOid,mergeable,mergeStateStatus,reviewDecision,isDraft,changedFiles,additions,deletions,labels)"
  files="$(gh pr diff "$pr" --repo "$REPO" --name-only || true)"
  checks="$(gh pr checks "$pr" --repo "$REPO" 2>&1 || true)"
  gh pr diff "$pr" --repo "$REPO" --patch >"$diff_file" 2>&1 || gh pr diff "$pr" --repo "$REPO" >"$diff_file" 2>&1 || true
  diff_line_count="$(wc -l <"$diff_file" | tr -d ' ')"
  if [[ "$CODEX_REVIEW_MAX_DIFF_LINES" =~ ^[0-9]+$ ]] && (( diff_line_count > CODEX_REVIEW_MAX_DIFF_LINES )); then
    diff_output="$(sed -n "1,${CODEX_REVIEW_MAX_DIFF_LINES}p" "$diff_file")
[diff truncated: ${diff_line_count} lines > ${CODEX_REVIEW_MAX_DIFF_LINES}]"
  else
    diff_output="$(cat "$diff_file")"
  fi
  mergeability_reviews="$(gh api "repos/${REPO}/issues/${pr}/comments" --paginate --jq '.[] | select((.body // "") | contains("takt-loop-mergeability-review")) | "created_at: \(.created_at)\n\(.body)\n---"' 2>&1 || true)"

  cat >"$prompt_file" <<EOF
You are the high-reasoning human-review substitute for this automation lane.

The normal automation merge gate returned HUMAN_REVIEW_REQUIRED or a local
size/path policy requires human review. Decide whether this PR can still
continue through the guarded merge path. Approve only when all of these are true:
- GitHub checks are green.
- The prior mergeability review approved the current head, either via GitHub
  reviewDecision or a takt-loop-mergeability-review PR comment whose Head SHA
  matches this PR and says Mergeable: YES.
- The diff is scoped to the original PR and contains no secrets, credentials,
  real customer/provider payloads, CI bypasses, admin policy changes, or unsafe
  dependency/script behavior.
- Package or manifest changes are low-risk and justified by the PR scope.
- Do not block solely because GitHub mergeability is dirty or unknown; guarded
  direct merge will convert merge conflicts into Mergeable: NO for review-fix.
  Block for safety, scope, security, or unreviewable-diff concerns.
- If the diff is truncated, block unless the visible diff, file list, metadata,
  and prior review make the PR low-risk enough to approve.

Return exactly this shape:

Codex-Human-Review: APPROVED|BLOCKED
Reason: one concise sentence
Blockers:
- none, or concrete blockers with file paths/commands
Verification:
- evidence used

Automation gate output:
\`\`\`
$(cat "$merge_log")
\`\`\`

PR metadata:
${metadata}

Changed files:
${files}

GitHub checks:
${checks}

Automation mergeability review comments:
${mergeability_reviews:-none}

PR diff:
\`\`\`diff
${diff_output}
\`\`\`
EOF

  if ! codex exec \
    --sandbox read-only \
    --cd "$(pwd)" \
    --model "$CODEX_REVIEW_MODEL" \
    -c "model_reasoning_effort=${CODEX_REVIEW_REASONING_EFFORT}" \
    -c 'approval_policy="never"' \
    --output-last-message "$review_file" \
    - <"$prompt_file" >"$codex_log" 2>&1; then
    cat >"$review_file" <<EOF
Codex-Human-Review: BLOCKED
Reason: codex high-reasoning review failed to run.
Blockers:
- Leave the PR open for manual inspection.
Verification:
- codex exec failed.
EOF
  fi

  if [[ ! -s "$review_file" ]]; then
    cat >"$review_file" <<EOF
Codex-Human-Review: BLOCKED
Reason: codex high-reasoning review produced no final response.
Blockers:
- Leave the PR open for manual inspection.
Verification:
- codex exec completed without writing an output-last-message file.
EOF
  fi

  if ! grep -Eq '^Codex-Human-Review:[[:space:]]*(APPROVED|BLOCKED)[[:space:]]*$' "$review_file"; then
    {
      printf 'Codex-Human-Review: BLOCKED\n'
      printf 'Reason: codex review output did not follow the required contract.\n'
      printf 'Blockers:\n'
      printf -- '- Normalize the review result before merging.\n'
      printf 'Verification:\n'
      printf -- '- Raw output was preserved below.\n\n'
      cat "$review_file"
    } >"${tmpdir}/review.normalized.md"
    mv "${tmpdir}/review.normalized.md" "$review_file"
  fi

  {
    printf '%s\n' "$CODEX_REVIEW_MARKER"
    printf 'Head SHA: `%s`\n' "$head_sha"
    printf 'Model: `%s`, reasoning: `%s`\n\n' "$CODEX_REVIEW_MODEL" "$CODEX_REVIEW_REASONING_EFFORT"
    cat "$review_file"
  } >"$body_file"

  gh pr comment "$pr" --repo "$REPO" --body-file "$body_file" >/dev/null 2>&1 || true

  if grep -Eq '^Codex-Human-Review:[[:space:]]*APPROVED[[:space:]]*$' "$review_file"; then
    log "codex high-reasoning review approved PR #${pr}"
    rm -rf "$tmpdir"
    return 0
  fi

  log "codex high-reasoning review blocked PR #${pr}"
  rm -rf "$tmpdir"
  return 1
}

merge_pr_if_safe() {
  local pr="$1"
  [[ "$AUTO_MERGE" == "1" ]] || {
    log "auto-merge disabled; leaving PR #${pr} open"
    return 0
  }

  log "waiting for checks on PR #${pr}"
  if ! gh pr checks "$pr" --repo "$REPO" --watch --interval 10; then
    comment_pr_mergeability_review "$pr" "GitHub checks failed or timed out before merge." || true
    log "PR #${pr} checks failed or timed out; not merging"
    return 0
  fi

  local head_sha codex_guard_approved=0
  head_sha="$(gh pr view "$pr" --repo "$REPO" --json headRefOid --jq '.headRefOid')"

  if ! pr_passes_local_merge_guard "$pr"; then
    local guard_log
    guard_log="$(mktemp)"
    {
      printf 'LOCAL_MERGE_GUARD_REQUIRED\n'
      printf 'PR exceeds configured file/line/path policy for automatic merge.\n\n'
      gh pr view "$pr" --repo "$REPO" --json number,title,url,mergeable,mergeStateStatus,changedFiles,additions,deletions
      printf '\nChanged files:\n'
      gh pr diff "$pr" --repo "$REPO" --name-only
    } >"$guard_log" 2>&1 || true

    if ! comment_pr_mergeability_review "$pr" "Local size/path merge guard requires human review."; then
      log "agy mergeability review did not approve PR #${pr}; leaving for review-fix"
      rm -f "$guard_log"
      return 0
    fi

    if codex_human_review_approves "$pr" "$head_sha" "$guard_log"; then
      codex_guard_approved=1
      log "local merge guard required human review, but codex high-reasoning review approved PR #${pr}"
    else
      block_pr_for_automation "$pr" "Local merge guard requires human review" "- PR exceeds configured file/line/path policy for automatic merge."
      rm -f "$guard_log"
      return 0
    fi
    rm -f "$guard_log"
  else
    if ! comment_pr_mergeability_review "$pr" "GitHub checks and local size/path merge guards passed."; then
      log "agy mergeability review did not approve PR #${pr}; not merging"
      return 0
    fi
  fi

  gh pr edit "$pr" --repo "$REPO" --add-label "$AUTO_MERGE_LABEL" >/dev/null

  local merge_log merge_status
  merge_log="$(mktemp)"
  set +e
  devloopd merge-if-safe --repo "$REPO" --pr "$pr" --expected-head "$head_sha" 2>&1 | tee "$merge_log"
  merge_status="${PIPESTATUS[0]}"
  set -e
  if (( merge_status == 0 )); then
    log "devloopd merge-if-safe accepted PR #${pr}"
    rm -f "$merge_log"
    return 0
  fi
  if grep -Fq "HUMAN_REVIEW_REQUIRED" "$merge_log"; then
    if (( codex_guard_approved == 1 )); then
      log "devloopd required human review after codex-approved local guard; continuing PR #${pr}"
    elif codex_human_review_approves "$pr" "$head_sha" "$merge_log"; then
      log "devloopd required human review, but codex high-reasoning review approved PR #${pr}"
    else
      block_pr_for_automation "$pr" "devloopd requires human review" "- devloopd merge-if-safe output:
\`\`\`
$(tail -40 "$merge_log")
\`\`\`"
      rm -f "$merge_log"
      return 0
    fi
  fi

  # Direct merge is still guarded by checks, size/path limits, agy YES, and head SHA.
  log "devloopd merge-if-safe did not accept PR #${pr}; attempting guarded direct merge"
  local direct_merge_log direct_merge_status
  direct_merge_log="$(mktemp)"
  set +e
  case "$MERGE_METHOD" in
    squash) gh pr merge "$pr" --repo "$REPO" --squash --delete-branch --match-head-commit "$head_sha" >"$direct_merge_log" 2>&1 ;;
    merge) gh pr merge "$pr" --repo "$REPO" --merge --delete-branch --match-head-commit "$head_sha" >"$direct_merge_log" 2>&1 ;;
    rebase) gh pr merge "$pr" --repo "$REPO" --rebase --delete-branch --match-head-commit "$head_sha" >"$direct_merge_log" 2>&1 ;;
    *)
      echo "unsupported TAKT_LOOP_MERGE_METHOD: ${MERGE_METHOD}" >&2
      set -e
      rm -f "$merge_log"
      rm -f "$direct_merge_log"
      return 1
      ;;
  esac
  direct_merge_status="$?"
  set -e
  rm -f "$merge_log"
  if (( direct_merge_status != 0 )); then
    cat "$direct_merge_log" >&2
    comment_pr_mergeability_blocker "$pr" "Guarded direct merge failed" "- gh pr merge output:
\`\`\`
$(tail -40 "$direct_merge_log")
\`\`\`"
    rm -f "$direct_merge_log"
    return 0
  fi
  rm -f "$direct_merge_log"
}

automation_pr_numbers() {
  gh pr list --repo "$REPO" --state open --limit 100 \
    --json number,isDraft,headRefName,author,labels \
    --jq '.[] | select(.isDraft | not) | select(.author.login != "dependabot[bot]") | select(.headRefName | test("^(takt|automation)/")) | select(([.labels[].name] | index("'"${BLOCKED_LABEL}"'")) | not) | .number'
}

review_pr_if_ready() {
  local pr="$1"

  if ! gh pr checks "$pr" --repo "$REPO" >/dev/null; then
    log "PR #${pr} checks are not passing yet; skipping mergeability review"
    return 0
  fi

  if ! pr_passes_local_merge_guard "$pr"; then
    comment_pr_mergeability_review "$pr" "Local size/path merge guard blocked the PR." || true
    log "PR #${pr} requires merge-stage high-reasoning review before automatic merge"
    return 0
  fi

  comment_pr_mergeability_review "$pr" "Periodic PR mergeability review. GitHub checks and local size/path merge guards passed." || true
}

review_open_prs() {
  ensure_labels

  local pr seen=0
  while IFS= read -r pr; do
    [[ -n "$pr" ]] || continue
    seen=1
    review_pr_if_ready "$pr"
  done < <(automation_pr_numbers)

  if (( seen == 0 )); then
    log "no open automation PR found for review"
  fi
}

latest_mergeability_review_body() {
  local pr="$1"
  gh api "repos/${REPO}/issues/${pr}/comments" --paginate \
    --jq "map(select((.body // \"\") | contains(\"${PR_REVIEW_MARKER}\"))) | last | .body // \"\""
}

fix_pr_from_review() {
  local pr="$1"
  ensure_labels

  local head_sha head_ref head_owner head_name head_repo review_body
  head_sha="$(gh pr view "$pr" --repo "$REPO" --json headRefOid --jq '.headRefOid')"
  head_ref="$(gh pr view "$pr" --repo "$REPO" --json headRefName --jq '.headRefName')"
  head_owner="$(gh pr view "$pr" --repo "$REPO" --json headRepositoryOwner --jq '.headRepositoryOwner.login // ""')"
  head_name="$(gh pr view "$pr" --repo "$REPO" --json headRepository --jq '.headRepository.name // ""')"
  head_repo="${head_owner}/${head_name}"

  if [[ "$head_repo" != "$REPO" ]]; then
    log "PR #${pr} is from ${head_repo}; skipping automatic review fix"
    return 0
  fi

  review_body="$(latest_mergeability_review_body "$pr")"
  if ! grep -Eq '^Mergeable:[[:space:]]*NO[[:space:]]*$' <<<"$review_body"; then
    log "PR #${pr} has no blocking mergeability review for automatic fix"
    return 0
  fi
  if ! grep -Fq "Head SHA: \`${head_sha}\`" <<<"$review_body"; then
    log "PR #${pr} review is stale for current head; skipping automatic fix"
    return 0
  fi

  local tmpdir worktree prompt_file
  tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/takt-review-fix-${pr}.XXXXXX")"
  worktree="${tmpdir}/worktree"
  prompt_file="${tmpdir}/prompt.md"

  git fetch origin "${head_ref}:refs/remotes/origin/${head_ref}" >/dev/null
  git worktree add --detach "$worktree" "origin/${head_ref}" >/dev/null

  cat >"$prompt_file" <<EOF
You are fixing PR #${pr} after an agy mergeability review.

Evaluate whether the review is valid before editing. If the review is not valid
or the fix would broaden the PR scope, do not change code.

Rules:
- Keep the diff scoped to PR #${pr} and its original issue.
- Use TDD when behavior changes.
- Do not touch secrets, credentials, real provider payloads, CI bypasses, or admin settings.
- For business logic, state, security, compatibility, memory, or performance decisions, add a short code comment explaining why.
- Run ./.takt/quality-gates/project-check.sh when practical.

Mergeability review:
${review_body}
EOF

  if ! cursor-agent --print --force --trust --model "$CURSOR_MODEL" --workspace "$worktree" "$(cat "$prompt_file")"; then
    log "cursor-agent failed while fixing PR #${pr}"
    block_pr_for_automation "$pr" "review fix command failed" "- cursor-agent did not complete the requested review fix."
    git worktree remove --force "$worktree" >/dev/null 2>&1 || true
    rm -rf "$tmpdir"
    return 0
  fi

  if ! run_quality_gate_for_worktree "$worktree"; then
    log "quality gate failed after review fix for PR #${pr}; leaving branch unpushed"
    block_pr_for_automation "$pr" "review fix quality gate failed" "- The automatic fix did not pass the local project quality gate."
    git worktree remove --force "$worktree" >/dev/null 2>&1 || true
    rm -rf "$tmpdir"
    return 0
  fi

  if [[ -z "$(git -C "$worktree" status --porcelain)" ]]; then
    log "cursor-agent made no changes for PR #${pr}"
    block_pr_for_automation "$pr" "review fix made no changes" "- The latest Mergeable: NO review did not produce a scoped automatic code change."
    git worktree remove --force "$worktree" >/dev/null 2>&1 || true
    rm -rf "$tmpdir"
    return 0
  fi

  git -C "$worktree" add -A
  git -C "$worktree" commit -m "fix: address PR #${pr} review"
  git -C "$worktree" push origin "HEAD:${head_ref}"

  log "pushed review fix for PR #${pr}"
  git worktree remove "$worktree" >/dev/null
  rm -rf "$tmpdir"
}

fix_open_prs_from_reviews() {
  ensure_labels

  local pr seen=0
  while IFS= read -r pr; do
    [[ -n "$pr" ]] || continue
    seen=1
    fix_pr_from_review "$pr"
  done < <(automation_pr_numbers)

  if (( seen == 0 )); then
    log "no open automation PR found for review fixes"
  fi
}

merge_open_prs() {
  ensure_labels

  local pr seen=0
  while IFS= read -r pr; do
    [[ -n "$pr" ]] || continue
    seen=1
    merge_pr_if_safe "$pr"
  done < <(automation_pr_numbers)

  if (( seen == 0 )); then
    log "no open automation PR found for merge"
  fi
}

run_once() {
  ensure_labels

  local issue
  if ! issue="$(mark_next_issue_ready)"; then
    if [[ "$CREATE_ISSUES" == "1" ]]; then
      if [[ -x "$ISSUE_CRAFTER" ]]; then
        log "idle: no safe issue ready; creating product issues"
        "$ISSUE_CRAFTER" create || true
        if issue="$(mark_next_issue_ready)"; then
          :
        else
          log "idle: no safe issue ready for automation"
          return 0
        fi
      else
        log "idle: create issues requested but ${ISSUE_CRAFTER} is not executable"
        return 0
      fi
    else
      log "idle: no safe issue ready for automation"
      return 0
    fi
  fi

  local start_log start_status
  start_log="$(mktemp)"

  clear_stale_active_runs

  log "running devloopd for issue #${issue}"
  set +e
  devloopd start \
    --repo "$REPO" \
    --workflow "$WORKFLOW" \
    --once \
    --max-active-runs "$MAX_ACTIVE_RUNS" \
    --stale-after-minutes "$ACTIVE_RUN_STALE_AFTER_MINUTES" \
    2>&1 | tee "$start_log"
  start_status="${PIPESTATUS[0]}"
  set -e
  if (( start_status != 0 )); then
    log "devloopd start exited with code ${start_status} for issue #${issue}"
  fi

  local pr
  pr="$(find_open_pr_for_issue "$issue")"
  if [[ -z "$pr" ]]; then
    log "no open PR found for issue #${issue}"
    block_issue_without_pr "$issue" "$start_status" "$start_log"
    rm -f "$start_log"
    return 0
  fi
  rm -f "$start_log"

  log "created/found PR #${pr} for issue #${issue}"
  merge_pr_if_safe "$pr"
}

case "${1:-once}" in
  once)
    run_once
    ;;
  loop)
    while true; do
      run_once || true
      log "sleeping ${INTERVAL_SECONDS}s"
      sleep "$INTERVAL_SECONDS"
    done
    ;;
  merge-pr)
    [[ -n "${2:-}" ]] || {
      echo "usage: $0 merge-pr <pr-number>" >&2
      exit 2
    }
    ensure_labels
    merge_pr_if_safe "$2"
    ;;
  review-pr)
    [[ -n "${2:-}" ]] || {
      echo "usage: $0 review-pr <pr-number>" >&2
      exit 2
    }
    ensure_labels
    review_pr_if_ready "$2"
    ;;
  review-open)
    review_open_prs
    ;;
  fix-pr)
    [[ -n "${2:-}" ]] || {
      echo "usage: $0 fix-pr <pr-number>" >&2
      exit 2
    }
    fix_pr_from_review "$2"
    ;;
  fix-open)
    fix_open_prs_from_reviews
    ;;
  merge-open)
    merge_open_prs
    ;;
  cleanup-stale-runs)
    clear_stale_active_runs
    ;;
  *)
    echo "usage: $0 [once|loop|review-open|fix-open|merge-open|cleanup-stale-runs|review-pr <pr-number>|fix-pr <pr-number>|merge-pr <pr-number>]" >&2
    exit 2
    ;;
esac
