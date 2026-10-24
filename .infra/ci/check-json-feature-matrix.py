#!/usr/bin/env python3
"""Check the standalone JSON feature fixture against selected serde_json versions."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path


MINIMUM_SERDE_JSON = "1.0.151"
FIXTURE_MANIFEST = Path("tests/fixtures/jcs_feature_unification/Cargo.toml")
COPIED_PATHS = (
    "Cargo.toml",
    "Cargo.lock",
    "src",
    "tests",
    "benches",
    "README.md",
    "LICENSE",
)
FEATURE_SETS = (
    "",
    "arbitrary_precision",
    "preserve_order",
    "arbitrary_precision preserve_order",
    "jcs",
    "jcs arbitrary_precision",
    "jcs preserve_order",
    "jcs arbitrary_precision preserve_order",
)


def run(command: list[str], *, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    print("$ " + " ".join(command), flush=True)
    try:
        return subprocess.run(command, check=True, text=True, env=env)
    except subprocess.CalledProcessError:
        print("Failed command: " + " ".join(command), flush=True)
        raise


def fixture_serde_json_version(manifest: Path, *, env: dict[str, str]) -> str:
    command = [
        "cargo",
        "metadata",
        "--locked",
        "--format-version",
        "1",
        "--manifest-path",
        str(manifest),
    ]
    print("$ " + " ".join(command), flush=True)
    result = subprocess.run(command, check=True, capture_output=True, text=True, env=env)
    metadata = json.loads(result.stdout)
    resolve = metadata.get("resolve")
    if resolve is None:
        raise RuntimeError("cargo metadata did not return a resolved dependency graph")

    root_ids = [
        node["id"]
        for node in resolve["nodes"]
        if node["id"].startswith("path+")
        and "#qubit-json-jcs-feature-unification@" in node["id"]
    ]
    if len(root_ids) != 1:
        raise RuntimeError(f"expected one fixture root package, found {root_ids!r}")

    nodes = {node["id"]: node for node in resolve["nodes"]}
    reachable = set()
    pending = list(root_ids)
    while pending:
        package_id = pending.pop()
        if package_id in reachable:
            continue
        reachable.add(package_id)
        node = nodes.get(package_id)
        if node is not None:
            pending.extend(dependency["pkg"] for dependency in node["deps"])

    versions = {
        package["version"]
        for package in metadata["packages"]
        if package["name"] == "serde_json" and package["id"] in reachable
    }
    if len(versions) != 1:
        raise RuntimeError(f"expected exactly one resolved serde_json version, found {sorted(versions)!r}")
    return versions.pop()


def copy_project(project_root: Path, destination: Path) -> None:
    for relative in COPIED_PATHS:
        source = project_root / relative
        target = destination / relative
        if source.is_dir():
            shutil.copytree(source, target)
        elif source.is_file():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        else:
            raise FileNotFoundError(f"required matrix input is missing: {source}")


def run_case(case: str, project_root: Path, temp_root: Path, target_dir: Path) -> None:
    case_parent = temp_root / case
    case_parent.mkdir()
    with tempfile.TemporaryDirectory(prefix=f"{case}-", dir=case_parent) as temporary:
        case_root = Path(temporary)
        copy_project(project_root, case_root)
        manifest = case_root / FIXTURE_MANIFEST
        environment = dict(os.environ)
        environment["CARGO_TARGET_DIR"] = str(target_dir)

        update = [
            "cargo",
            "update",
            "--manifest-path",
            str(manifest),
            "-p",
            "serde_json",
        ]
        if case == "minimum":
            update.extend(("--precise", MINIMUM_SERDE_JSON))
        run(update, env=environment)

        resolved_version = fixture_serde_json_version(manifest, env=environment)
        if case == "minimum" and resolved_version != MINIMUM_SERDE_JSON:
            raise RuntimeError(
                f"minimum case resolved serde_json {resolved_version}, expected {MINIMUM_SERDE_JSON}"
            )

        print(f"Case {case}: serde_json {resolved_version}", flush=True)
        for features in FEATURE_SETS:
            print(f"Testing serde_json {resolved_version}; features: {features or '(default)'}", flush=True)
            command = [
                "cargo",
                "test",
                "--locked",
                "--manifest-path",
                str(manifest),
            ]
            if features:
                command.extend(("--features", features))
            run(command, env=environment)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", choices=("minimum", "latest", "all"), default="all")
    arguments = parser.parse_args()

    project_root = Path(__file__).resolve().parents[2]
    cases = ("minimum", "latest") if arguments.case == "all" else (arguments.case,)
    with tempfile.TemporaryDirectory(prefix="qubit-json-feature-matrix-") as temporary:
        temp_root = Path(temporary)
        target_dir = temp_root / "target"
        for case in cases:
            run_case(case, project_root, temp_root, target_dir)


if __name__ == "__main__":
    main()
