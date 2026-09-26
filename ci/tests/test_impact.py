from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
IMPACT_TOOL = REPOSITORY_ROOT / "ci" / "impact.py"
CONFIGURATION = REPOSITORY_ROOT / "ci" / "config" / "impact.json"
ADAPTER = REPOSITORY_ROOT / "ci" / "adapters" / "rust.json"


def load_impact_module():
    spec = importlib.util.spec_from_file_location("qzt_ci_impact", IMPACT_TOOL)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load impact tool from {IMPACT_TOOL}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def git(repository: Path, *arguments: str) -> str:
    completed = subprocess.run(
        ["git", *arguments],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


class ImpactPlanTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.repository = Path(self.temporary_directory.name)
        git(self.repository, "init", "--initial-branch=main")
        git(self.repository, "config", "user.email", "ci-test@example.invalid")
        git(self.repository, "config", "user.name", "CI test")

        self.write("Cargo.toml", "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n")
        self.write("Cargo.lock", "version = 4\n")
        self.write(
            "src/lib.rs",
            "mod reader;\nmod search;\npub use reader::Reader;\npub use search::search;\n",
        )
        self.write("src/reader.rs", "pub struct Reader;\n")
        self.write(
            "src/search.rs",
            "use crate::reader::Reader;\npub fn search() { let _ = Reader; }\n",
        )
        self.write("tests/phase0_smoke.rs", "#[test]\nfn starts() {}\n")
        self.write(
            "tests/reader_contract.rs",
            "use fixture::reader::Reader;\n#[test]\nfn reads() { let _ = Reader; }\n",
        )
        self.write(
            "tests/search_contract.rs",
            "use fixture::search::search;\n#[test]\nfn searches() { search(); }\n",
        )
        git(self.repository, "add", ".")
        git(self.repository, "commit", "-m", "fixture baseline")
        self.base_revision = git(self.repository, "rev-parse", "HEAD")

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write(self, relative_path: str, contents: str) -> None:
        path = self.repository / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")

    def commit(self, message: str) -> str:
        git(self.repository, "add", "-A")
        git(self.repository, "commit", "-m", message)
        return git(self.repository, "rev-parse", "HEAD")

    def plan(
        self,
        base_revision: str,
        head_revision: str,
        configuration_path: Path = CONFIGURATION,
    ):
        impact = load_impact_module()
        return impact.create_plan(
            repository=self.repository,
            base_revision=base_revision,
            head_revision=head_revision,
            configuration_path=configuration_path,
            adapter_path=ADAPTER,
        )

    def custom_configuration(self, **updates) -> Path:
        configuration = json.loads(CONFIGURATION.read_text(encoding="utf-8"))
        configuration.update(updates)
        path = self.repository / "impact-config.json"
        path.write_text(json.dumps(configuration), encoding="utf-8")
        return path

    def test_changed_module_selects_direct_and_reverse_dependent_tests(self) -> None:
        self.write("src/reader.rs", "pub struct Reader { pub offset: u64 }\n")
        head_revision = self.commit("change reader")

        plan = self.plan(self.base_revision, head_revision)

        self.assertEqual(plan["strategy"], "selective")
        self.assertFalse(plan["fallback"])
        self.assertEqual(plan["affectedModules"], ["reader", "search"])
        self.assertEqual(
            plan["integrationTestTargets"],
            ["reader_contract", "search_contract"],
        )
        self.assertEqual(plan["smokeTestTargets"], ["phase0_smoke"])

    def test_grouped_rust_imports_propagate_to_reverse_dependents(self) -> None:
        self.write(
            "src/search.rs",
            "use crate::{reader::Reader};\npub fn search() { let _ = Reader; }\n",
        )
        self.base_revision = self.commit("use grouped import")
        self.write("src/reader.rs", "pub struct Reader { pub offset: u64 }\n")
        head_revision = self.commit("change grouped dependency")

        plan = self.plan(self.base_revision, head_revision)

        self.assertIn("search", plan["affectedModules"])
        self.assertIn("search_contract", plan["integrationTestTargets"])

    def test_public_reexport_usage_selects_the_owning_module_test(self) -> None:
        self.write(
            "tests/public_api.rs",
            "use fixture::Reader;\n#[test]\nfn public_reader() { let _ = Reader; }\n",
        )
        self.base_revision = self.commit("add public API test")
        self.write("src/reader.rs", "pub struct Reader { pub offset: u64 }\n")
        head_revision = self.commit("change public reader")

        plan = self.plan(self.base_revision, head_revision)

        self.assertIn("public_api", plan["integrationTestTargets"])

    def test_manual_module_mapping_covers_binary_only_tests(self) -> None:
        self.write("src/main.rs", "fn main() {}\n")
        self.write("tests/cli_contract.rs", "#[test]\nfn cli_starts() {}\n")
        self.base_revision = self.commit("add binary contract")
        self.write("src/main.rs", "fn main() { println!(\"fixture\"); }\n")
        head_revision = self.commit("change binary")
        configuration = self.custom_configuration(
            moduleTestMappings={"main": ["cli_contract"]}
        )

        plan = self.plan(self.base_revision, head_revision, configuration)

        self.assertIn("cli_contract", plan["e2eTestTargets"])

    def test_manual_path_mapping_covers_documentation_contracts(self) -> None:
        self.write("docs/guide.md", "baseline\n")
        self.write("tests/docs_contract.rs", "#[test]\nfn docs_are_current() {}\n")
        self.base_revision = self.commit("add docs contract")
        self.write("docs/guide.md", "updated\n")
        head_revision = self.commit("change docs")
        configuration = self.custom_configuration(
            pathTestMappings={"docs/**": ["docs_contract"]}
        )

        plan = self.plan(self.base_revision, head_revision, configuration)

        self.assertIn("docs_contract", plan["integrationTestTargets"])

    def test_missing_manual_mapping_target_falls_back_to_full_tests(self) -> None:
        self.write("docs/guide.md", "baseline\n")
        self.base_revision = self.commit("add guide")
        self.write("docs/guide.md", "updated\n")
        head_revision = self.commit("change guide")
        configuration = self.custom_configuration(
            pathTestMappings={"docs/**": ["missing_contract"]}
        )

        plan = self.plan(self.base_revision, head_revision, configuration)

        self.assertEqual(plan["strategy"], "full")
        self.assertIn("missing_contract", plan["fallbackReason"])

    def test_lock_file_change_requires_full_test_fallback(self) -> None:
        self.write("Cargo.lock", "version = 4\n# dependency update\n")
        head_revision = self.commit("change lock file")

        plan = self.plan(self.base_revision, head_revision)

        self.assertEqual(plan["strategy"], "full")
        self.assertTrue(plan["fallback"])
        self.assertIn("Cargo.lock", plan["fallbackReason"])

    def test_deleted_test_is_not_passed_to_the_test_runner(self) -> None:
        (self.repository / "tests" / "reader_contract.rs").unlink()
        head_revision = self.commit("remove obsolete reader test")

        plan = self.plan(self.base_revision, head_revision)

        all_targets = (
            plan["unitTestTargets"]
            + plan["integrationTestTargets"]
            + plan["e2eTestTargets"]
            + plan["smokeTestTargets"]
        )
        self.assertNotIn("reader_contract", all_targets)
        self.assertIn("tests/reader_contract.rs", plan["deletedFiles"])

    def test_rename_records_old_path_but_only_selects_existing_target(self) -> None:
        git(
            self.repository,
            "mv",
            "tests/reader_contract.rs",
            "tests/reader_regression.rs",
        )
        head_revision = self.commit("rename reader test")

        plan = self.plan(self.base_revision, head_revision)

        self.assertIn("tests/reader_contract.rs", plan["deletedFiles"])
        self.assertIn("tests/reader_regression.rs", plan["changedFiles"])
        self.assertIn("reader_regression", plan["integrationTestTargets"])
        self.assertNotIn("reader_contract", plan["integrationTestTargets"])

    def test_unresolvable_base_revision_falls_back_to_full_tests(self) -> None:
        plan = self.plan("0" * 40, git(self.repository, "rev-parse", "HEAD"))

        self.assertEqual(plan["strategy"], "full")
        self.assertTrue(plan["fallback"])
        self.assertIn("base revision", plan["fallbackReason"])

    def test_symbolic_revision_is_resolved_to_a_canonical_commit(self) -> None:
        self.write("src/search.rs", "pub fn search() {}\n")
        head_revision = self.commit("change search from symbolic base")
        git(self.repository, "branch", "baseline", self.base_revision)

        plan = self.plan("baseline", head_revision)

        self.assertEqual(plan["baseRevision"], self.base_revision)
        self.assertEqual(plan["headRevision"], head_revision)

    def test_cli_writes_machine_readable_plan(self) -> None:
        self.write("src/search.rs", "pub fn search() {}\n")
        head_revision = self.commit("change search")
        output_path = self.repository / "impact-plan.json"

        subprocess.run(
            [
                "python3",
                str(IMPACT_TOOL),
                "plan",
                "--repository",
                str(self.repository),
                "--base",
                self.base_revision,
                "--head",
                head_revision,
                "--config",
                str(CONFIGURATION),
                "--adapter",
                str(ADAPTER),
                "--output",
                str(output_path),
            ],
            check=True,
        )

        plan = json.loads(output_path.read_text(encoding="utf-8"))
        self.assertEqual(plan["strategy"], "selective")
        self.assertEqual(plan["headRevision"], head_revision)

    def test_cli_can_force_full_validation_for_non_pr_events(self) -> None:
        revision = git(self.repository, "rev-parse", "HEAD")
        output_path = self.repository / "impact-plan.json"

        subprocess.run(
            [
                "python3",
                str(IMPACT_TOOL),
                "plan",
                "--repository",
                str(self.repository),
                "--base",
                revision,
                "--head",
                revision,
                "--config",
                str(CONFIGURATION),
                "--adapter",
                str(ADAPTER),
                "--output",
                str(output_path),
                "--force-full",
                "non-PR CI requires full validation",
            ],
            check=True,
        )

        plan = json.loads(output_path.read_text(encoding="utf-8"))
        self.assertEqual(plan["strategy"], "full")
        self.assertTrue(plan["fallback"])
        self.assertEqual(
            plan["fallbackReason"], "non-PR CI requires full validation"
        )

    def test_invalid_impact_configuration_writes_a_full_fallback_plan(self) -> None:
        revision = git(self.repository, "rev-parse", "HEAD")
        invalid_configuration = self.repository / "invalid-impact.json"
        invalid_configuration.write_text("{not json", encoding="utf-8")
        output_path = self.repository / "impact-plan.json"

        completed = subprocess.run(
            [
                "python3",
                str(IMPACT_TOOL),
                "plan",
                "--repository",
                str(self.repository),
                "--base",
                revision,
                "--head",
                revision,
                "--config",
                str(invalid_configuration),
                "--adapter",
                str(ADAPTER),
                "--output",
                str(output_path),
            ],
            check=False,
        )

        self.assertEqual(completed.returncode, 0)
        plan = json.loads(output_path.read_text(encoding="utf-8"))
        self.assertEqual(plan["strategy"], "full")
        self.assertIn("configuration", plan["fallbackReason"])


class WorkflowContractTests(unittest.TestCase):
    def test_ci_routes_prs_through_impact_analysis_and_keeps_full_lanes(self) -> None:
        workflow = (REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn('branches: [main, "release/**"]', workflow)
        self.assertIn("schedule:", workflow)
        self.assertIn("workflow_dispatch:", workflow)
        self.assertIn("python3 ci/impact.py plan", workflow)
        self.assertIn("python3 ci/impact.py run", workflow)
        self.assertIn("fetch-depth: 0", workflow)

    def test_all_manual_mapping_targets_exist(self) -> None:
        configuration = json.loads(CONFIGURATION.read_text(encoding="utf-8"))
        existing_targets = {path.stem for path in (REPOSITORY_ROOT / "tests").glob("*.rs")}
        configured_targets = {
            target
            for mapping_name in ("moduleTestMappings", "pathTestMappings")
            for targets in configuration[mapping_name].values()
            for target in targets
        }

        self.assertEqual(configured_targets - existing_targets, set())


class ImpactRunnerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.repository = Path(self.temporary_directory.name)
        self.record_path = self.repository / "commands.jsonl"
        self.runner_path = self.repository / "record_command.py"
        self.runner_path.write_text(
            """
import json
import sys
from pathlib import Path

record = Path(sys.argv[1])
with record.open("a", encoding="utf-8") as stream:
    stream.write(json.dumps(sys.argv[2:]) + "\\n")
if "fail" in sys.argv[2:]:
    raise SystemExit(7)
""".strip()
            + "\n",
            encoding="utf-8",
        )
        self.adapter_path = self.repository / "adapter.json"
        self.adapter_path.write_text(
            json.dumps(
                {
                    "id": "fixture",
                    "fullTestCommand": [
                        sys.executable,
                        str(self.runner_path),
                        str(self.record_path),
                        "full",
                    ],
                    "integrationTestCommand": [
                        sys.executable,
                        str(self.runner_path),
                        str(self.record_path),
                        "integration",
                        "{target}",
                    ],
                    "unitTestCommand": [
                        sys.executable,
                        str(self.runner_path),
                        str(self.record_path),
                        "unit",
                        "{target}",
                    ],
                }
            ),
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def recorded_commands(self) -> list[list[str]]:
        return [
            json.loads(line)
            for line in self.record_path.read_text(encoding="utf-8").splitlines()
        ]

    def test_full_plan_runs_only_the_adapter_full_test_command(self) -> None:
        impact = load_impact_module()
        plan = {
            "strategy": "full",
            "unitTestTargets": [],
            "integrationTestTargets": [],
            "e2eTestTargets": [],
            "smokeTestTargets": [],
        }

        exit_code, summary = impact.run_plan(
            repository=self.repository,
            plan=plan,
            adapter_path=self.adapter_path,
        )

        self.assertEqual(exit_code, 0)
        self.assertEqual(self.recorded_commands(), [["full"]])
        self.assertEqual(summary["total"], 1)
        self.assertEqual(summary["succeeded"], 1)
        self.assertEqual(summary["failed"], 0)
        self.assertEqual(summary["skipped"], 0)

    def test_selective_plan_deduplicates_targets_and_records_failures(self) -> None:
        impact = load_impact_module()
        plan = {
            "strategy": "selective",
            "unitTestTargets": ["lib"],
            "integrationTestTargets": ["reader", "fail"],
            "e2eTestTargets": ["cli"],
            "smokeTestTargets": ["reader"],
        }

        exit_code, summary = impact.run_plan(
            repository=self.repository,
            plan=plan,
            adapter_path=self.adapter_path,
        )

        self.assertEqual(exit_code, 1)
        self.assertEqual(
            self.recorded_commands(),
            [
                ["unit", "lib"],
                ["integration", "reader"],
                ["integration", "fail"],
                ["integration", "cli"],
            ],
        )
        self.assertEqual(summary["total"], 4)
        self.assertEqual(summary["succeeded"], 3)
        self.assertEqual(summary["failed"], 1)
        self.assertEqual(summary["skipped"], 1)
        self.assertGreaterEqual(summary["durationSeconds"], 0)


if __name__ == "__main__":
    unittest.main()
