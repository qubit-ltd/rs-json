// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Verifies allocation-free scalar lexeme measurements through public encoding.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_json::text::encode_to_vec;

/// Verifies integer and floating point measurements match emitted JSON text.
#[test]
fn test_scalar_lexeme_limits_match_json_output() {
    let mut integers = JsonEncodeSession::owned(
        JsonEncodeLimits::empty().with_max_number_bytes(40),
    );
    assert!(encode_to_vec(&i128::MIN, &mut integers).is_ok());

    let mut floats = JsonEncodeSession::owned(
        JsonEncodeLimits::empty().with_max_number_bytes(4),
    );
    assert!(encode_to_vec(&1.25_f64, &mut floats).is_ok());
}
