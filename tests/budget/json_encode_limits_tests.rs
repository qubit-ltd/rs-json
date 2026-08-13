// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for JSON encoding limits.

use qubit_budget::ResourceLimit;
use qubit_json::JsonEncodeLimits;
use qubit_json::JsonResource;
use qubit_json::JsonValueLimits;

/// Verifies encoding limits retain directional and value configurations.
#[test]
fn test_json_encode_limits_retain_configured_values() {
    let value_limits = JsonValueLimits::empty();
    let limits = JsonEncodeLimits::empty()
        .with_output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 12))
        .with_value_limits(value_limits);

    assert_eq!(limits.max_output_bytes(), Some(12));
    assert_eq!(
        limits.output_bytes_limit().map(ResourceLimit::maximum),
        Some(12)
    );
    assert_eq!(limits.value_limits(), value_limits);
}
