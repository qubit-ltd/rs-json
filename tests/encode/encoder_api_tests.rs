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

/// Verifies the limits constructor builds a cumulative session from explicit
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

/// Verifies mutable access and ownership transfer preserve encoder accounting
/// state.
#[test]
fn test_json_encoder_session_mut_and_into_session_preserve_usage() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(4)
        .build();
    let mut encoder = JsonEncoder::with_limits(limits);
    let mut attempt = encoder.session_mut().begin_value();
    attempt
        .try_consume_output_bytes(1)
        .expect("the direct output charge should fit");
    attempt.commit().expect("the empty value transaction should commit");

    let session = encoder.into_session();
    assert_eq!(session.output_budget().expect("configured output budget").used(), 1);
}
