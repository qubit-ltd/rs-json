// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression tests for budgeted JSON object keys.

use std::collections::BTreeMap;

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_json::text::encode_to_vec;

/// Verifies object keys consume the key-byte budget.
#[test]
fn test_budgeted_key_checks_key_bytes() {
    let values = BTreeMap::from([(String::from("key"), true)]);
    let limits = JsonEncodeLimits::empty().with_max_key_bytes(2);
    let mut session = JsonEncodeSession::owned(limits);

    assert!(encode_to_vec(&values, &mut session).is_err());
}
