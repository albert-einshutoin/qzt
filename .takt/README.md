# Default TAKT Loop Template

This directory is a portable template for local subscription-only TAKT/devloopd
automation. Copy it into a project as `.takt`, then tune only the project
identity and verification commands.

## Install

```bash
cd /path/to/project
mkdir -p .takt
cp -R /Volumes/Satechi/Developer/agent-scrum/.takt/. .takt/
chmod +x .takt/automation/full-auto-devloop.sh .takt/quality-gates/project-check.sh
chmod +x .takt/automation/create-product-issues.sh .takt/automation/staged-devloop.sh
```

Set the repository once per shell when auto-detection is not enough:

```bash
export TAKT_LOOP_REPO=owner/repo
```

`TAKT_LOOP_*` is the generic prefix for new projects.

## Required Project Edits

1. Update `.takt/config.yaml` if the base branch is not `main`.
2. Update `.takt/quality-gates/project-check.sh` for the project's real test,
   lint, build, security, and packaging commands.
3. Update forbidden paths and size limits in `.takt/automation/full-auto-devloop.sh`
   when the project has sensitive directories outside the defaults.
4. Keep `.takt/.gitignore` so run logs, caches, and local state do not enter git.

## One Issue

```bash
devloopd run \
  --issue 123 \
  --repo "${TAKT_LOOP_REPO:-owner/repo}" \
  --workflow .takt/workflows/subscription-devloop.yaml \
  --verbose
```

## One Scan Cycle

```bash
devloopd start \
  --repo "${TAKT_LOOP_REPO:-owner/repo}" \
  --workflow .takt/workflows/subscription-devloop.yaml \
  --once
```

## Full Auto Loop

```bash
.takt/automation/full-auto-devloop.sh once
.takt/automation/full-auto-devloop.sh loop
```

The loop:

1. creates required automation labels,
2. marks one safe issue with `agent:ready`,
3. runs `devloopd start --once`,
4. waits for PR checks,
5. posts an agy mergeability review for the current PR head,
6. merges only when checks, local guards, and review all pass.

Disable merge while validating a new project:

```bash
TAKT_LOOP_AUTO_MERGE=0 .takt/automation/full-auto-devloop.sh loop
```

Tune conservative auto-merge limits:

```bash
TAKT_LOOP_MAX_AUTO_MERGE_FILES=20 \
TAKT_LOOP_MAX_AUTO_MERGE_LINES=800 \
.takt/automation/full-auto-devloop.sh once
```

To also create new low-risk issues when no safe issue exists, enable the
generic issue crafter:

```bash
TAKT_LOOP_CREATE_ISSUES=1 \
.takt/automation/full-auto-devloop.sh loop
```

The issue crafter reads bounded project sources from README/docs/tasks by
default. Tune it per project when those are not the right product inputs:

```bash
TAKT_LOOP_PROJECT_NAME=my-project \
TAKT_LOOP_ISSUE_SOURCE_PATHS='README.md:docs:tasks' \
.takt/automation/create-product-issues.sh plan
```

For lower-noise continuous operation, use the staged scheduler. It separates
issue scouting, issue-to-PR, PR review, review-fix, and merge stages:

```bash
.takt/automation/staged-devloop.sh once
.takt/automation/staged-devloop.sh loop
```

## Agent Routing

- `codex-cli` / `gpt-5.5`: product-safe planning and final arbitration
- `cursor-cli` / `composer-2.5`: primary TDD implementation
- `opencode-cli` / `opencode-go/minimax-m3`: cheap verification fixes and hygiene review
- `agy-cli` / `Gemini 3.5 Flash (High)`: mergeability and security review
