#!/usr/bin/env python3
"""Fail-safe, adapter-driven change impact analysis for CI test selection."""

from __future__ import annotations

import argparse
import fnmatch
import importlib.util
import json
import re
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Iterable


def _git(repository: Path, *arguments: str) -> bytes:
    return subprocess.run(
        ["git", *arguments],
        cwd=repository,
        check=True,
        capture_output=True,
    ).stdout


def _resolve_revision(repository: Path, revision: str) -> str | None:
    if not revision or revision.startswith("-") or "\0" in revision:
        return None
    completed = subprocess.run(
        [
            "git",
            "rev-parse",
            "--verify",
            "--end-of-options",
            f"{revision}^{{commit}}",
        ],
        cwd=repository,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        return None
    commit = completed.stdout.strip()
    return commit if re.fullmatch(r"[0-9a-fA-F]{40,64}", commit) else None


def _matches(path: str, patterns: Iterable[str]) -> bool:
    return any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns)


def _fallback_plan(
    *,
    base_revision: str,
    head_revision: str,
    reason: str,
    adapter: dict[str, Any],
    changed_files: list[str] | None = None,
    deleted_files: list[str] | None = None,
) -> dict[str, Any]:
    return {
        "strategy": "full",
        "baseRevision": base_revision,
        "headRevision": head_revision,
        "detectedProjects": [f"{adapter.get('projectType', 'unknown')}:."],
        "adapter": adapter.get("id", "unknown"),
        "changedFiles": changed_files or [],
        "deletedFiles": deleted_files or [],
        "affectedProjects": ["."],
        "affectedModules": [],
        "unitTestTargets": [],
        "integrationTestTargets": [],
        "e2eTestTargets": [],
        "smokeTestTargets": [],
        "fallback": True,
        "fallbackReason": reason,
    }


def _changed_paths(
    repository: Path, base_revision: str, head_revision: str
) -> tuple[list[str], list[str]]:
    raw = _git(
        repository,
        "diff",
        "--name-status",
        "-z",
        "-M",
        "-C",
        f"{base_revision}...{head_revision}",
    )
    fields = raw.decode("utf-8").split("\0")
    if fields and fields[-1] == "":
        fields.pop()

    changed: set[str] = set()
    deleted: set[str] = set()
    index = 0
    while index < len(fields):
        status = fields[index]
        index += 1
        if status.startswith(("R", "C")):
            if index + 1 >= len(fields):
                raise ValueError("incomplete rename or copy record from git diff")
            old_path, new_path = fields[index], fields[index + 1]
            index += 2
            changed.add(new_path)
            if status.startswith("R"):
                deleted.add(old_path)
            continue
        if index >= len(fields):
            raise ValueError("incomplete path record from git diff")
        path = fields[index]
        index += 1
        if status.startswith("D"):
            deleted.add(path)
        else:
            changed.add(path)
    return sorted(changed), sorted(deleted)


def _load_analysis_adapter(adapter_path: Path, adapter: dict[str, Any]):
    adapter_directory = adapter_path.resolve().parent
    module_path = (adapter_directory / adapter["analysisModule"]).resolve()
    try:
        module_path.relative_to(adapter_directory)
    except ValueError as error:
        raise ValueError("analysis adapter must stay in its adapter directory") from error
    spec = importlib.util.spec_from_file_location(
        f"ci_impact_adapter_{adapter['id']}", module_path
    )
    if spec is None or spec.loader is None:
        raise ValueError(f"cannot load analysis adapter: {module_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    if not callable(getattr(module, "analyze", None)):
        raise ValueError(f"analysis adapter has no analyze function: {module_path}")
    return module


def _target_name(path: str) -> str:
    target = Path(path).stem
    if not re.fullmatch(r"[A-Za-z0-9_-]+", target):
        raise ValueError(f"unsafe test target name derived from {path}")
    return target


def create_plan(
    *,
    repository: Path,
    base_revision: str,
    head_revision: str,
    configuration_path: Path,
    adapter_path: Path,
) -> dict[str, Any]:
    repository = repository.resolve()
    with configuration_path.open(encoding="utf-8") as stream:
        configuration = json.load(stream)
    with adapter_path.open(encoding="utf-8") as stream:
        adapter = json.load(stream)

    requested_base_revision = base_revision
    requested_head_revision = head_revision
    resolved_base_revision = _resolve_revision(repository, base_revision)
    if resolved_base_revision is None:
        return _fallback_plan(
            base_revision=base_revision,
            head_revision=head_revision,
            reason=f"base revision is unavailable: {requested_base_revision}",
            adapter=adapter,
        )
    resolved_head_revision = _resolve_revision(repository, head_revision)
    if resolved_head_revision is None:
        return _fallback_plan(
            base_revision=resolved_base_revision,
            head_revision=head_revision,
            reason=f"head revision is unavailable: {requested_head_revision}",
            adapter=adapter,
        )
    base_revision = resolved_base_revision
    head_revision = resolved_head_revision

    try:
        changed_files, deleted_files = _changed_paths(
            repository, base_revision, head_revision
        )
    except (OSError, subprocess.CalledProcessError, UnicodeDecodeError, ValueError) as error:
        return _fallback_plan(
            base_revision=base_revision,
            head_revision=head_revision,
            reason=f"change detection failed: {error}",
            adapter=adapter,
        )

    all_changed_paths = sorted(set(changed_files + deleted_files))
    dangerous_path = next(
        (
            path
            for path in all_changed_paths
            if _matches(
                path,
                configuration["fullTestGlobs"] + adapter["dangerousGlobs"],
            )
        ),
        None,
    )
    if dangerous_path is not None:
        return _fallback_plan(
            base_revision=base_revision,
            head_revision=head_revision,
            reason=f"full-test rule matched: {dangerous_path}",
            adapter=adapter,
            changed_files=changed_files,
            deleted_files=deleted_files,
        )

    project_markers = adapter["projectMarkers"]
    if not all((repository / marker).exists() for marker in project_markers):
        return _fallback_plan(
            base_revision=base_revision,
            head_revision=head_revision,
            reason=f"project markers are missing: {', '.join(project_markers)}",
            adapter=adapter,
            changed_files=changed_files,
            deleted_files=deleted_files,
        )

    supported_globs = (
        adapter["sourceGlobs"]
        + adapter["testGlobs"]
        + configuration["safeNonCodeGlobs"]
    )
    unsupported_path = next(
        (path for path in all_changed_paths if not _matches(path, supported_globs)), None
    )
    if unsupported_path is not None:
        return _fallback_plan(
            base_revision=base_revision,
            head_revision=head_revision,
            reason=f"unclassified changed file: {unsupported_path}",
            adapter=adapter,
            changed_files=changed_files,
            deleted_files=deleted_files,
        )

    try:
        analysis_adapter = _load_analysis_adapter(adapter_path, adapter)
        analysis = analysis_adapter.analyze(
            repository=repository,
            changed_files=changed_files,
            deleted_files=deleted_files,
            adapter=adapter,
        )
        affected_modules = sorted(set(analysis["affectedModules"]))
        adapter_selected_tests = set(analysis["selectedTestPaths"])
        test_files = sorted(set(analysis["testFiles"]))
        unit_targets = sorted(set(analysis["unitTestTargets"]))
    except (OSError, UnicodeDecodeError, ValueError, KeyError) as error:
        return _fallback_plan(
            base_revision=base_revision,
            head_revision=head_revision,
            reason=f"dependency graph generation failed: {error}",
            adapter=adapter,
            changed_files=changed_files,
            deleted_files=deleted_files,
        )

    direct_changed_tests = {
        path
        for path in changed_files
        if _matches(path, adapter["testGlobs"])
        and (repository / path).is_file()
    }
    selected_tests = set(direct_changed_tests) | adapter_selected_tests
    path_mapped_targets = {
        target
        for path in all_changed_paths
        for pattern, targets in configuration.get("pathTestMappings", {}).items()
        if _matches(path, [pattern])
        for target in targets
    }
    supplemental_mappings = configuration.get("moduleTestMappings", {})
    module_mapped_targets = {
        target
        for module in affected_modules
        for target in supplemental_mappings.get(module, [])
    }
    existing_test_targets: set[str] = set()
    for relative_path in test_files:
        test_path = repository / relative_path
        if not test_path.is_file():
            raise ValueError(f"adapter returned a missing test file: {relative_path}")
        test_target = _target_name(relative_path)
        existing_test_targets.add(test_target)
        if test_target in path_mapped_targets | module_mapped_targets:
            selected_tests.add(relative_path)

    missing_mapped_targets = sorted(
        (path_mapped_targets | module_mapped_targets) - existing_test_targets
    )
    if missing_mapped_targets:
        return _fallback_plan(
            base_revision=base_revision,
            head_revision=head_revision,
            reason="manual test mapping targets are missing: "
            + ", ".join(missing_mapped_targets),
            adapter=adapter,
            changed_files=changed_files,
            deleted_files=deleted_files,
        )

    integration_targets: list[str] = []
    e2e_targets: list[str] = []
    for path in sorted(selected_tests):
        target = _target_name(path)
        if _matches(path, configuration["e2eTestGlobs"]):
            e2e_targets.append(target)
        else:
            integration_targets.append(target)

    smoke_targets = sorted(set(configuration["smokeTestTargets"]))
    integration_targets = sorted(set(integration_targets) - set(smoke_targets))
    e2e_targets = sorted(set(e2e_targets) - set(smoke_targets))
    selected_target_count = len(
        unit_targets + integration_targets + e2e_targets + smoke_targets
    )
    if all_changed_paths and selected_target_count == 0:
        return _fallback_plan(
            base_revision=base_revision,
            head_revision=head_revision,
            reason="changes were detected but no test targets were selected",
            adapter=adapter,
            changed_files=changed_files,
            deleted_files=deleted_files,
        )

    return {
        "strategy": "selective",
        "baseRevision": base_revision,
        "headRevision": head_revision,
        "detectedProjects": [f"{adapter['projectType']}:."],
        "adapter": adapter["id"],
        "changedFiles": changed_files,
        "deletedFiles": deleted_files,
        "affectedProjects": ["."],
        "affectedModules": affected_modules,
        "unitTestTargets": unit_targets,
        "integrationTestTargets": integration_targets,
        "e2eTestTargets": e2e_targets,
        "smokeTestTargets": smoke_targets,
        "fallback": False,
        "fallbackReason": None,
    }


def _render_command(template: list[str], target: str | None) -> list[str]:
    if not template or not all(isinstance(argument, str) for argument in template):
        raise ValueError("adapter command must be a non-empty string array")
    if target is not None and not re.fullmatch(r"[A-Za-z0-9_-]+", target):
        raise ValueError(f"unsafe test target: {target}")
    return [
        argument.replace("{target}", target or "") for argument in template
    ]


def run_plan(
    *, repository: Path, plan: dict[str, Any], adapter_path: Path
) -> tuple[int, dict[str, Any]]:
    with adapter_path.open(encoding="utf-8") as stream:
        adapter = json.load(stream)

    started_at = time.monotonic()
    commands: list[tuple[str, str | None, list[str]]] = []
    skipped = 0
    if plan.get("strategy") == "full":
        commands.append(
            ("full", None, _render_command(adapter["fullTestCommand"], None))
        )
    elif plan.get("strategy") == "selective":
        seen: set[str] = set()
        categories = (
            ("unit", "unitTestTargets", "unitTestCommand"),
            ("integration", "integrationTestTargets", "integrationTestCommand"),
            (
                "e2e",
                "e2eTestTargets",
                "e2eTestCommand"
                if "e2eTestCommand" in adapter
                else "integrationTestCommand",
            ),
            (
                "smoke",
                "smokeTestTargets",
                "smokeTestCommand"
                if "smokeTestCommand" in adapter
                else "integrationTestCommand",
            ),
        )
        for category, plan_key, command_key in categories:
            for target in plan.get(plan_key, []):
                if target in seen:
                    skipped += 1
                    continue
                seen.add(target)
                commands.append(
                    (
                        category,
                        target,
                        _render_command(adapter[command_key], target),
                    )
                )
    else:
        raise ValueError(f"unsupported test strategy: {plan.get('strategy')}")

    results: list[dict[str, Any]] = []
    for category, target, command in commands:
        command_started_at = time.monotonic()
        print(
            f"Running {category} target {target or 'all'}: "
            + " ".join(command),
            flush=True,
        )
        # Adapter data is trusted repository configuration, but targets still
        # travel as argv entries so changed filenames can never become shell code.
        completed = subprocess.run(command, cwd=repository, check=False)
        results.append(
            {
                "category": category,
                "target": target,
                "exitCode": completed.returncode,
                "durationSeconds": round(time.monotonic() - command_started_at, 3),
            }
        )

    failed = sum(result["exitCode"] != 0 for result in results)
    summary = {
        "strategy": plan["strategy"],
        "total": len(results),
        "succeeded": len(results) - failed,
        "failed": failed,
        "skipped": skipped,
        "durationSeconds": round(time.monotonic() - started_at, 3),
        "results": results,
    }
    print("Test execution summary: " + json.dumps(summary, sort_keys=True))
    return (1 if failed else 0), summary


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    plan_parser = subparsers.add_parser("plan", help="create a test impact plan")
    plan_parser.add_argument("--repository", type=Path, default=Path.cwd())
    plan_parser.add_argument("--base", required=True)
    plan_parser.add_argument("--head", required=True)
    plan_parser.add_argument("--config", type=Path, required=True)
    plan_parser.add_argument("--adapter", type=Path, required=True)
    plan_parser.add_argument("--output", type=Path, required=True)
    plan_parser.add_argument(
        "--force-full",
        metavar="REASON",
        help="force full validation for main, release, scheduled, or manual CI",
    )
    run_parser = subparsers.add_parser("run", help="execute a test impact plan")
    run_parser.add_argument("--repository", type=Path, default=Path.cwd())
    run_parser.add_argument("--plan", type=Path, required=True)
    run_parser.add_argument("--adapter", type=Path, required=True)
    run_parser.add_argument("--summary", type=Path, required=True)
    return parser


def main() -> int:
    arguments = _build_parser().parse_args()
    if arguments.command == "plan":
        try:
            if arguments.force_full:
                with arguments.adapter.open(encoding="utf-8") as stream:
                    adapter = json.load(stream)
                plan = _fallback_plan(
                    base_revision=arguments.base,
                    head_revision=arguments.head,
                    reason=arguments.force_full,
                    adapter=adapter,
                )
            else:
                plan = create_plan(
                    repository=arguments.repository,
                    base_revision=arguments.base,
                    head_revision=arguments.head,
                    configuration_path=arguments.config,
                    adapter_path=arguments.adapter,
                )
        except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
            # A broken impact rule must expand coverage. Only an unreadable
            # adapter is fatal because no trustworthy full command then exists.
            try:
                with arguments.adapter.open(encoding="utf-8") as stream:
                    adapter = json.load(stream)
                plan = _fallback_plan(
                    base_revision=arguments.base,
                    head_revision=arguments.head,
                    reason=f"impact analysis configuration failed: {error}",
                    adapter=adapter,
                )
            except (OSError, ValueError, KeyError, json.JSONDecodeError) as adapter_error:
                print(
                    f"impact adapter configuration error: {adapter_error}",
                    file=sys.stderr,
                )
                return 2
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(
            json.dumps(plan, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(json.dumps(plan, indent=2, sort_keys=True))
        return 0
    if arguments.command == "run":
        try:
            with arguments.plan.open(encoding="utf-8") as stream:
                plan = json.load(stream)
            exit_code, summary = run_plan(
                repository=arguments.repository,
                plan=plan,
                adapter_path=arguments.adapter,
            )
        except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
            print(f"test plan execution error: {error}", file=sys.stderr)
            return 2
        arguments.summary.parent.mkdir(parents=True, exist_ok=True)
        arguments.summary.write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        return exit_code
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
