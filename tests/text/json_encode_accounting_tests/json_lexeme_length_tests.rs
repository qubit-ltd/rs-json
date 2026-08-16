// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests allocation-free scalar lexeme measurements through public encoding.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;

use crate::text::json_encode_test_support::encode;

/// Verifies integer and floating point measurements match emitted JSON text.
#[test]
fn test_scalar_lexeme_limits_match_json_output() {
    let mut integers = JsonEncodeSession::owned(
        JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_number_bytes(40)
            .build(),
    );
    assert!(encode(&i128::MIN, &mut integers).is_ok());

    let mut floats = JsonEncodeSession::owned(
        JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_number_bytes(4)
            .build(),
    );
    assert!(encode(&1.25_f64, &mut floats).is_ok());
}
