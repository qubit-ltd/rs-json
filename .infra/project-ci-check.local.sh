#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$project_root"
cargo clippy --workspace --all-targets -- -D warnings
cargo test --locked --test tests --features serde_json/preserve_order value::traverse::json_tree_mutator_tests
cargo test --manifest-path fuzz/Cargo.toml --lib --tests --locked
cargo test --manifest-path tests/fixtures/documentation_examples/Cargo.toml --doc --locked
python3 .infra/ci/check-json-feature-matrix.py
