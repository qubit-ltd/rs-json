// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests nested value serialization through the public encoder.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncoder;

/// Verifies nested values re-enter the budget-aware encoder traversal.
#[test]
fn test_budgeted_value_serializes_nested_values() {
    let session = JsonEncodeSession::owned(JsonEncodeLimits::<JsonResource, usize>::builder().build());
    let output = JsonEncoder::new(session)
        .to_vec(&vec![vec![1_u8]])
        .expect("nested values should encode");

    assert_eq!(output, b"[[1]]");
}
