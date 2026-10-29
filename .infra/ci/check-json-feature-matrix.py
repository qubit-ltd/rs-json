#!/usr/bin/env python3
"""Test the root crate against selected serde_json versions and feature sets."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path


BASELINE_SERDE_JSON = "1.0.135"
SERDE_JSON_REGISTRY_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
COPIED_PATHS = (
    "Cargo.toml",
    "Cargo.lock",
    "src",
    "tests",
    "benches",
    "README.md",
    "README.zh_CN.md",
    "doc",
    "LICENSE",
)
FEATURE_SETS = (
    "",
    "jcs",
    "serde_json/arbitrary_precision",
    "serde_json/preserve_order",
    "serde_json/arbitrary_precision,serde_json/preserve_order",
    "jcs,serde_json/arbitrary_precision",
    "jcs,serde_json/preserve_order",
    "jcs,serde_json/arbitrary_precision,serde_json/preserve_order",
)


def run(command: list[str], *, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    print("$ " + " ".join(command), flush=True)
    try:
        return subprocess.run(command, check=True, text=True, env=env)
    except subprocess.CalledProcessError:
        print("Failed command: " + " ".join(command), flush=True)
        raise


def resolved_serde_json(manifest: Path, *, env: dict[str, str]) -> tuple[str, str | None]:
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
    root_id = None if resolve is None else resolve.get("root")
    if root_id is None:
        raise RuntimeError("cargo metadata did not return the root package")

    packages = {package["id"]: package for package in metadata["packages"]}
    root_package = packages.get(root_id)
    if root_package is None or root_package["name"] != "qubit-json":
        name = None if root_package is None else root_package["name"]
        raise RuntimeError(f"expected root package qubit-json, found {name!r}")

    nodes = {node["id"]: node for node in resolve["nodes"]}
    reachable = set()
    pending = [root_id]
    while pending:
        package_id = pending.pop()
        if package_id in reachable:
            continue
        reachable.add(package_id)
        node = nodes.get(package_id)
        if node is not None:
            pending.extend(dependency["pkg"] for dependency in node["deps"])

    serde_json = [
        package for package_id, package in packages.items()
        if package_id in reachable and package["name"] == "serde_json"
    ]
    if len(serde_json) != 1:
        versions = sorted(package["version"] for package in serde_json)
        raise RuntimeError(f"expected exactly one reachable serde_json version, found {versions!r}")
    return serde_json[0]["version"], serde_json[0].get("source")


def require_crates_io_source(source: str | None) -> None:
    if source != SERDE_JSON_REGISTRY_SOURCE:
        raise RuntimeError(
            f"serde_json must resolve from crates.io, found {source or 'local path'}; "
            "refusing to label an overridden package as a registry version"
        )


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
        manifest = case_root / "Cargo.toml"
        environment = dict(os.environ)
        environment["CARGO_TARGET_DIR"] = str(target_dir)

        original_version, original_source = resolved_serde_json(manifest, env=environment)
        require_crates_io_source(original_source)
        print(
            f"Case {case}: starting serde_json {original_version}; "
            f"source: {original_source or 'local path'}",
            flush=True,
        )

        update = [
            "cargo",
            "update",
            "--manifest-path",
            str(manifest),
            "-p",
            "serde_json",
        ]
        if case == "minimum":
            update.extend(("--precise", BASELINE_SERDE_JSON))
        run(update, env=environment)

        resolved_version, resolved_source = resolved_serde_json(manifest, env=environment)
        if resolved_source != original_source:
            raise RuntimeError(
                f"case {case} changed serde_json source from {original_source!r} "
                f"to {resolved_source!r}; refusing to test a different dependency source"
            )
        require_crates_io_source(resolved_source)
        if case == "minimum" and resolved_version != BASELINE_SERDE_JSON:
            raise RuntimeError(
                f"baseline case resolved serde_json {resolved_version}, "
                f"expected test baseline {BASELINE_SERDE_JSON}"
            )

        print(
            f"Case {case}: serde_json {resolved_version}; "
            f"meaning: {'test baseline' if case == 'minimum' else 'latest compatible'}",
            flush=True,
        )
        for features in FEATURE_SETS:
            label = features or "(default)"
            print(f"Case {case}; serde_json {resolved_version}; features: {label}", flush=True)
            command = [
                "cargo",
                "test",
                "--locked",
                "--manifest-path",
                str(manifest),
                "--lib",
                "--test",
                "tests",
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
