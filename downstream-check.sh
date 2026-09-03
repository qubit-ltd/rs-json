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
# Compile every direct rs-json consumer in a sibling rust-common checkout.
#

set -euo pipefail

PROJECT_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
COMMON_ROOT=$(dirname "$PROJECT_ROOT")

CRATES=(rs-config rs-datatype rs-http rs-metadata rs-redact rs-value)
FEATURES=("" json "" json json json)

present=0
for crate in "${CRATES[@]}"; do
    if [ -f "$COMMON_ROOT/$crate/Cargo.toml" ]; then
        present=$((present + 1))
    fi
done

if [ "$present" -eq 0 ]; then
    echo "Direct downstream checks skipped: no sibling rs-* checkouts found."
    exit 0
fi

if [ "$present" -ne "${#CRATES[@]}" ]; then
    echo "error: direct downstream checkout is incomplete under $COMMON_ROOT" >&2
    for crate in "${CRATES[@]}"; do
        if [ ! -f "$COMMON_ROOT/$crate/Cargo.toml" ]; then
            echo "  missing: $crate" >&2
        fi
    done
    exit 1
fi

for index in "${!CRATES[@]}"; do
    crate=${CRATES[$index]}
    feature=${FEATURES[$index]}
    args=(check --all-targets)
    if [ -n "$feature" ]; then
        args+=(--features "$feature")
    fi

    echo "Checking direct downstream crate: $crate"
    if [ -n "${RS_CI_BUILD_TOOLCHAIN:-}" ]; then
        cargo "+$RS_CI_BUILD_TOOLCHAIN" "${args[@]}" --manifest-path "$COMMON_ROOT/$crate/Cargo.toml"
    else
        cargo "${args[@]}" --manifest-path "$COMMON_ROOT/$crate/Cargo.toml"
    fi
done

echo "All direct downstream crates compile against the current rs-json checkout."
