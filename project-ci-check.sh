#!/bin/bash
################################################################################
#
#    Copyright (c) 2026 Haixing Hu.
#
#    SPDX-License-Identifier: Apache-2.0
#
#    Licensed under the Apache License, Version 2.0.
#
################################################################################
#
# Project-specific checks not covered by the shared rs-infra pipeline.
#

set -euo pipefail

PROJECT_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

"$PROJECT_ROOT/doc-example-check.sh"

if [ -n "${RUSTUP_TOOLCHAIN:-}" ]; then
    cargo +"$RUSTUP_TOOLCHAIN" test \
        --manifest-path "$PROJECT_ROOT/fuzz/Cargo.toml"
    cargo +"$RUSTUP_TOOLCHAIN" bench \
        --manifest-path "$PROJECT_ROOT/Cargo.toml" \
        --bench decoder_bench -- --test
else
    cargo test --manifest-path "$PROJECT_ROOT/fuzz/Cargo.toml"
    cargo bench --manifest-path "$PROJECT_ROOT/Cargo.toml" --bench decoder_bench -- --test
fi
