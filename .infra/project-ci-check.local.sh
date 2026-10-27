#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$project_root"
cargo clippy --workspace --all-targets -- -D warnings
cargo test --locked --test tests --features serde_json/preserve_order value::traverse::json_tree_mutator_tests
feature_sets=(
    ""
    "jcs"
    "serde_json/arbitrary_precision"
    "serde_json/preserve_order"
    "serde_json/arbitrary_precision,serde_json/preserve_order"
    "jcs,serde_json/arbitrary_precision"
    "jcs,serde_json/preserve_order"
    "jcs,serde_json/arbitrary_precision,serde_json/preserve_order"
)
for json_features in "${feature_sets[@]}"; do
    if [[ -z "$json_features" ]]; then
        cargo test --locked --lib
    else
        cargo test --locked --lib --features "$json_features"
    fi
done
