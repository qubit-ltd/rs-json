// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for JSON decoding limits.

use qubit_budget::ResourceLimit;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;

/// Verifies decoding limits retain directional and value configurations.
#[test]
fn test_json_decode_limits_retain_configured_values() {
    let value_limits = JsonValueLimits::empty();
    let limits = JsonDecodeLimits::empty()
        .with_input_bytes_limit(ResourceLimit::new(
            JsonResource::InputBytes,
            12,
        ))
        .with_value_limits(value_limits);

    assert_eq!(limits.max_input_bytes(), Some(12));
    assert_eq!(
        limits.input_bytes_limit().map(ResourceLimit::maximum),
        Some(12)
    );
    assert_eq!(limits.value_limits(), &value_limits);
}
