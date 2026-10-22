#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$project_root"
cargo test --manifest-path tests/fixtures/documentation_examples/Cargo.toml --doc --locked
cargo test --manifest-path tests/fixtures/jcs_feature_unification/Cargo.toml --locked
cargo test --manifest-path tests/fixtures/jcs_feature_unification/Cargo.toml --locked --features arbitrary_precision
cargo test --manifest-path tests/fixtures/jcs_feature_unification/Cargo.toml --locked --features preserve_order
cargo test --manifest-path tests/fixtures/jcs_feature_unification/Cargo.toml --locked --all-features
