"""Regression tests for the root-crate serde_json feature matrix."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "check-json-feature-matrix.py"
SPEC = importlib.util.spec_from_file_location("check_json_feature_matrix", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
matrix = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(matrix)


def metadata(
    version: str = "1.0.135",
    root_name: str = "qubit-json",
    source: str = "registry+https://github.com/rust-lang/crates.io-index",
) -> str:
    root_id = f"path+file:///tmp/project#qubit-json@0.12.0"
    serde_id = f"registry+https://github.com/rust-lang/crates.io-index#serde_json@{version}"
    return json.dumps(
        {
            "packages": [
                {"id": root_id, "name": root_name, "version": "0.12.0"},
                {
                    "id": serde_id,
                    "name": "serde_json",
                    "version": version,
                    "source": source,
                },
            ],
            "resolve": {
                "root": root_id,
                "nodes": [
                    {"id": root_id, "deps": [{"pkg": serde_id}]},
                    {"id": serde_id, "deps": []},
                ],
            },
        }
    )


class JsonFeatureMatrixTests(unittest.TestCase):
    def make_project(self, root: Path) -> None:
        for relative in matrix.COPIED_PATHS:
            path = root / relative
            if relative in {"src", "tests", "benches", "doc"}:
                path.mkdir(parents=True, exist_ok=True)
                (path / "marker.txt").write_text(relative)
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("locked bytes" if relative == "Cargo.lock" else relative)
        (root / ".git").mkdir()
        (root / ".git" / "marker").write_text("must not copy")
        (root / "target").mkdir()
        (root / "target" / "marker").write_text("must not copy")

    def completed(self, command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
        if command[1] == "metadata":
            return subprocess.CompletedProcess(command, 0, metadata(), "")
        return subprocess.CompletedProcess(command, 0, "", "")

    def test_copy_project_includes_required_inputs_and_preserves_lock(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "project"
            destination = Path(temporary) / "copy"
            self.make_project(root)
            original_lock = (root / "Cargo.lock").read_bytes()

            matrix.copy_project(root, destination)

            self.assertEqual((destination / "Cargo.lock").read_bytes(), original_lock)
            self.assertTrue((destination / "README.zh_CN.md").is_file())
            self.assertTrue((destination / "doc" / "marker.txt").is_file())
            self.assertFalse((destination / ".git").exists())
            self.assertFalse((destination / "target").exists())

    def test_two_cases_run_root_manifest_and_all_eight_feature_sets(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "project"
            temp_root = Path(temporary) / "matrix"
            target_dir = temp_root / "target"
            root.mkdir()
            temp_root.mkdir()
            self.make_project(root)

            with patch.object(matrix.subprocess, "run", side_effect=self.completed) as mocked_run:
                matrix.run_case("minimum", root, temp_root, target_dir)
                matrix.run_case("latest", root, temp_root, target_dir)

            commands = [call.args[0] for call in mocked_run.call_args_list]
            updates = [command for command in commands if len(command) > 1 and command[1] == "update"]
            tests = [command for command in commands if len(command) > 1 and command[1] == "test"]
            self.assertEqual(len(updates), 2)
            self.assertEqual(len(tests), 16)
            self.assertIn("--precise", updates[0])
            self.assertNotIn("--precise", updates[1])
            for command in updates + tests:
                manifest = Path(command[command.index("--manifest-path") + 1])
                self.assertEqual(manifest.name, "Cargo.toml")
                self.assertNotIn("jcs_feature_unification", str(manifest))
            self.assertTrue(all("--lib" in command and "tests" in command for command in tests))
            observed_features = [
                command[command.index("--features") + 1] if "--features" in command else ""
                for command in tests
            ]
            self.assertEqual(observed_features[:8], list(matrix.FEATURE_SETS))
            self.assertEqual(observed_features[8:], list(matrix.FEATURE_SETS))

    def test_update_must_preserve_the_serde_json_source(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "project"
            temp_root = Path(temporary) / "matrix"
            root.mkdir()
            temp_root.mkdir()
            self.make_project(root)
            metadata_calls = 0

            def change_source(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                nonlocal metadata_calls
                if command[1] == "metadata":
                    metadata_calls += 1
                    source = (
                        "path+file:///tmp/serde_json"
                        if metadata_calls == 2
                        else "registry+https://github.com/rust-lang/crates.io-index"
                    )
                    return subprocess.CompletedProcess(command, 0, metadata(source=source), "")
                return subprocess.CompletedProcess(command, 0, "", "")

            with patch.object(matrix.subprocess, "run", side_effect=change_source):
                with self.assertRaisesRegex(RuntimeError, "changed serde_json source"):
                    matrix.run_case("minimum", root, temp_root, temp_root / "target")

    def test_path_override_is_rejected_before_version_update(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "project"
            temp_root = Path(temporary) / "matrix"
            root.mkdir()
            temp_root.mkdir()
            self.make_project(root)

            def path_source(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                return subprocess.CompletedProcess(
                    command, 0, metadata(source="path+file:///tmp/serde_json"), ""
                )

            with patch.object(matrix.subprocess, "run", side_effect=path_source) as mocked_run:
                with self.assertRaisesRegex(RuntimeError, "must resolve from crates.io"):
                    matrix.run_case("latest", root, temp_root, temp_root / "target")

            self.assertEqual(mocked_run.call_count, 1)
            self.assertEqual(mocked_run.call_args.args[0][1], "metadata")

    def test_metadata_must_resolve_the_configured_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "project"
            temp_root = Path(temporary) / "matrix"
            root.mkdir()
            temp_root.mkdir()
            self.make_project(root)

            with patch.object(
                matrix.subprocess,
                "run",
                side_effect=lambda command, **kwargs: subprocess.CompletedProcess(
                    command, 0, metadata("1.0.134"), ""
                ) if command[1] == "metadata" else subprocess.CompletedProcess(command, 0, "", ""),
            ):
                with self.assertRaisesRegex(RuntimeError, "expected test baseline 1.0.135"):
                    matrix.run_case("minimum", root, temp_root, temp_root / "target")

    def test_failed_test_command_propagates(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "project"
            temp_root = Path(temporary) / "matrix"
            root.mkdir()
            temp_root.mkdir()
            self.make_project(root)

            def fail_test(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                if command[1] == "test":
                    raise subprocess.CalledProcessError(101, command)
                if command[1] == "metadata":
                    return subprocess.CompletedProcess(command, 0, metadata(), "")
                return subprocess.CompletedProcess(command, 0, "", "")

            with patch.object(matrix.subprocess, "run", side_effect=fail_test):
                with self.assertRaises(subprocess.CalledProcessError):
                    matrix.run_case("minimum", root, temp_root, temp_root / "target")


if __name__ == "__main__":
    unittest.main()
