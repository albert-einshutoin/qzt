"""Rust/Cargo impact analysis adapter for the language-neutral CI planner."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any


def _module_name(path: str) -> str | None:
    candidate = Path(path)
    if candidate.parent != Path("src") or candidate.suffix != ".rs":
        return None
    return candidate.stem


def _module_dependencies(repository: Path) -> dict[str, set[str]]:
    source_directory = repository / "src"
    if not source_directory.is_dir():
        raise ValueError("source directory is missing")

    dependencies: dict[str, set[str]] = {}
    for source_path in sorted(source_directory.glob("*.rs")):
        module = source_path.stem
        contents = source_path.read_text(encoding="utf-8")
        direct = set(
            re.findall(
                r"(?:use|pub\s+use)\s+crate::([A-Za-z_][A-Za-z0-9_]*)",
                contents,
            )
        )
        direct.update(
            re.findall(r"crate::([A-Za-z_][A-Za-z0-9_]*)::", contents)
        )
        for grouped_import in re.findall(
            r"(?:use|pub\s+use)\s+crate::\{(.*?)\}\s*;",
            contents,
            re.DOTALL,
        ):
            direct.update(
                re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*)::", grouped_import)
            )
        direct.discard(module)
        dependencies[module] = direct
    return dependencies


def _reverse_dependents(
    changed_modules: set[str], dependencies: dict[str, set[str]]
) -> list[str]:
    # Walk dependents, not only imports in the changed file: a low-level change
    # must exercise every higher layer that can observe its behavior.
    affected = set(changed_modules)
    while True:
        discovered = {
            module
            for module, direct_dependencies in dependencies.items()
            if module not in affected and direct_dependencies.intersection(affected)
        }
        if not discovered:
            return sorted(affected)
        affected.update(discovered)


def _public_exports(repository: Path) -> dict[str, set[str]]:
    library_path = repository / "src" / "lib.rs"
    if not library_path.is_file():
        return {}
    contents = library_path.read_text(encoding="utf-8")
    exports: dict[str, set[str]] = {}
    for match in re.finditer(
        r"pub\s+use\s+([A-Za-z_][A-Za-z0-9_]*)::(\{.*?\}|[A-Za-z_][A-Za-z0-9_]*)\s*;",
        contents,
        re.DOTALL,
    ):
        module, raw_symbols = match.groups()
        entries = (
            raw_symbols[1:-1].split(",")
            if raw_symbols.startswith("{")
            else [raw_symbols]
        )
        module_exports = exports.setdefault(module, set())
        for entry in entries:
            identifiers = re.findall(r"[A-Za-z_][A-Za-z0-9_]*", entry)
            if identifiers:
                module_exports.add(identifiers[0])
                if "as" in identifiers and len(identifiers) >= 3:
                    module_exports.add(identifiers[-1])
    return exports


def _test_modules(
    test_path: Path,
    crate_names: set[str],
    public_exports: dict[str, set[str]],
) -> set[str]:
    contents = test_path.read_text(encoding="utf-8")
    crate_expression = "|".join(re.escape(name) for name in sorted(crate_names))
    if not crate_expression:
        return set()
    modules = set(
        re.findall(
            rf"(?:{crate_expression})::([A-Za-z_][A-Za-z0-9_]*)",
            contents,
        )
    )
    grouped_imports = re.findall(
        rf"use\s+(?:{crate_expression})::\{{(.*?)\}}\s*;",
        contents,
        re.DOTALL,
    )
    for module, symbols in public_exports.items():
        for symbol in symbols:
            direct_use = re.search(
                rf"(?:{crate_expression})::{re.escape(symbol)}\b", contents
            )
            grouped_use = any(
                re.search(rf"\b{re.escape(symbol)}\b", body)
                for body in grouped_imports
            )
            if direct_use or grouped_use:
                modules.add(module)
                break
    return modules


def analyze(
    *,
    repository: Path,
    changed_files: list[str],
    deleted_files: list[str],
    adapter: dict[str, Any],
) -> dict[str, Any]:
    all_changed_paths = set(changed_files + deleted_files)
    changed_modules = {
        module
        for path in all_changed_paths
        if (module := _module_name(path)) is not None
    }
    affected_modules = _reverse_dependents(
        changed_modules, _module_dependencies(repository)
    )

    crate_names = set(adapter.get("crateNames", []))
    cargo_manifest = (repository / "Cargo.toml").read_text(encoding="utf-8")
    package_match = re.search(
        r'^name\s*=\s*"([^"]+)"', cargo_manifest, re.MULTILINE
    )
    if package_match:
        crate_names.add(package_match.group(1).replace("-", "_"))

    public_exports = _public_exports(repository)
    test_files = [
        path.relative_to(repository).as_posix()
        for path in sorted((repository / "tests").glob("*.rs"))
    ]
    selected_tests = [
        relative_path
        for relative_path in test_files
        if _test_modules(repository / relative_path, crate_names, public_exports).intersection(
            affected_modules
        )
    ]
    return {
        "affectedModules": affected_modules,
        "selectedTestPaths": selected_tests,
        "testFiles": test_files,
        "unitTestTargets": ["lib"] if changed_modules else [],
    }
