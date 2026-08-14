// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression tests for private serde_json value adapters.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_json::text::encode_to_vec;
use serde_json::Number;
use serde_json::from_str;

/// Verifies arbitrary-precision numbers use the number-byte budget.
#[test]
fn test_budgeted_private_value_checks_number_bytes() {
    let number: Number = from_str("123456789").expect("number should parse");
    let limits = JsonEncodeLimits::empty().with_max_number_bytes(8);
    let mut session = JsonEncodeSession::owned(limits);

    assert!(encode_to_vec(&number, &mut session).is_err());
}
