// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Verifies the complete JSON budget API exported by qubit-json.

use qubit_json::JsonDecodeLimits;
use qubit_json::JsonDecodeSession;
use qubit_json::JsonEncodeLimits;
use qubit_json::JsonEncodeSession;
use qubit_json::JsonResource;
use qubit_json::JsonValueBudget;
use qubit_json::JsonValueLimits;

#[test]
fn test_json_budget_api_is_exported_from_qubit_json() {
    let value_limits = JsonValueLimits::<JsonResource, usize>::empty();
    let value_budget = JsonValueBudget::new(value_limits);
    assert_eq!(value_budget.limits(), &value_limits);

    let decode = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let encode = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    assert_eq!(decode.max_input_bytes(), None);
    assert_eq!(encode.max_output_bytes(), None);
}
