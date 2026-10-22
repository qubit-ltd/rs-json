#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$project_root"
exec cargo test --manifest-path tests/fixtures/documentation_examples/Cargo.toml --doc --locked
