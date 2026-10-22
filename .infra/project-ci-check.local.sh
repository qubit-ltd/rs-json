#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$project_root"
cargo clippy --workspace --all-targets -- -D warnings
cargo test --manifest-path tests/fixtures/documentation_examples/Cargo.toml --doc --locked
fixture=tests/fixtures/jcs_feature_unification/Cargo.toml
cargo test --manifest-path "$fixture" --locked
for feature_set in \
    arbitrary_precision \
    preserve_order \
    "arbitrary_precision preserve_order" \
    jcs \
    "jcs arbitrary_precision" \
    "jcs preserve_order" \
    "jcs arbitrary_precision preserve_order"
do
    cargo test --manifest-path "$fixture" --locked --features "$feature_set"
done
