// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the budget-aware JSON serializer.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_json::text::encode_to_vec;

/// Verifies scalar serialization uses the wrapped JSON serializer.
#[test]
fn test_json_encode_serializer_serializes_scalar_values() {
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());

    assert_eq!(
        encode_to_vec(&true, &mut session)
            .expect("scalar JSON should serialize"),
        b"true"
    );
}
