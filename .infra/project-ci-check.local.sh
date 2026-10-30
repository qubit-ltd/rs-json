#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$project_root"
python3 -B -m unittest discover -s .infra/ci/tests -p 'test_*.py'
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
        cargo test --locked --lib --test tests
    else
        cargo test --locked --lib --test tests --features "$json_features"
    fi
done
cargo test --locked --doc
cargo test --locked --doc --features jcs
cargo test --locked --manifest-path fuzz/Cargo.toml --lib --tests
cargo test --locked --manifest-path fuzz/Cargo.toml --features arbitrary-precision --lib --test json_reference_tests --test json_budget_invariants_tests --test json_tree_invariants_tests --test unique_decode_reference_tests --test json_unique_decode_invariants_tests
cargo test --locked --manifest-path fuzz/Cargo.toml --features preserve-order --lib --test json_reference_tests --test json_budget_invariants_tests --test json_tree_invariants_tests --test unique_decode_reference_tests --test json_unique_decode_invariants_tests
cargo test --locked --manifest-path fuzz/Cargo.toml --features arbitrary-precision,preserve-order --lib --test json_reference_tests --test json_budget_invariants_tests --test json_tree_invariants_tests --test unique_decode_reference_tests --test json_unique_decode_invariants_tests
python3 .infra/ci/check-json-feature-matrix.py --case all
