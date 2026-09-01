#!/bin/bash
set -euo pipefail

PROJECT_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
exec env CARGO_TARGET_DIR="$PROJECT_ROOT/target/documentation-examples" \
    cargo test --doc --manifest-path "$PROJECT_ROOT/tests/fixtures/documentation_examples/Cargo.toml"
