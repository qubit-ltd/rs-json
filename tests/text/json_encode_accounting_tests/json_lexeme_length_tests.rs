// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests allocation-free scalar lexeme measurements through public encoding.

use std::fmt::Debug;

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use serde::Serialize;

use crate::text::json_encode_test_support::encode;

/// Verifies one finite number is admitted exactly at its compact output size.
fn assert_number_lexeme_budget<T>(value: T)
where
    T: Debug + Serialize,
{
    let expected = serde_json::to_vec(&value).expect("finite test number must serialize");
    let mut exact = JsonEncodeSession::owned(
        JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_number_bytes(expected.len())
            .build(),
    );
    let encoded = encode(&value, &mut exact).expect("exact number-byte budget must admit the value");
    assert_eq!(encoded, expected, "compact output must match serde_json for {value:?}");

    let mut short = JsonEncodeSession::owned(
        JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_number_bytes(expected.len() - 1)
            .build(),
    );
    assert!(
        encode(&value, &mut short).is_err(),
        "one byte below the compact output must reject {value:?}",
    );
}

/// Verifies integer and floating point measurements match emitted JSON text.
#[test]
fn test_scalar_lexeme_limits_match_json_output() {
    let mut integers = JsonEncodeSession::owned(
        JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_number_bytes(20)
            .build(),
    );
    assert!(encode(&u64::MAX, &mut integers).is_ok());

    let mut floats = JsonEncodeSession::owned(
        JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_number_bytes(4)
            .build(),
    );
    assert!(encode(&1.25_f64, &mut floats).is_ok());
}

/// Verifies finite `f64` edge cases use serde_json's exact compact length.
#[test]
fn test_f64_lexeme_limits_match_serde_json_output() {
    for value in [
        0.0_f64,
        -0.0_f64,
        f64::MIN_POSITIVE,
        f64::MIN_POSITIVE / 2.0,
        f64::MAX,
        1.0e-7_f64,
    ] {
        assert_number_lexeme_budget(value);
    }
}

/// Verifies finite `f32` edge cases use serde_json's exact compact length.
#[test]
fn test_f32_lexeme_limits_match_serde_json_output() {
    for value in [0.0_f32, -0.0_f32, f32::MIN_POSITIVE, f32::MAX, 1.0e-7_f32] {
        assert_number_lexeme_budget(value);
    }
}
