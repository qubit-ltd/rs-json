// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Verifies explicit strict encoder construction.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncoder;

/// Verifies the owned constructor builds a cumulative session from explicit
/// limits.
#[test]
fn test_json_encoder_owned_uses_explicit_limits() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(8)
        .max_nodes(2)
        .build();
    let encoder = JsonEncoder::with_limits(limits);

    assert_eq!(encoder.session().max_output_bytes(), Some(8));
    assert_eq!(encoder.session().value_budget().limits().max_nodes(), Some(2));
}

/// Verifies unlimited construction is explicit and leaves every budget
/// unconfigured.
#[test]
fn test_json_encoder_unlimited_has_no_limits() {
    let encoder = JsonEncoder::unlimited();

    assert_eq!(encoder.session().max_output_bytes(), None);
    assert_eq!(encoder.session().value_budget().limits().max_nodes(), None);
}
