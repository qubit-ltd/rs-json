// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests bounded JSON output buffering.

use qubit_budget::ResourceLimit;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncodeError;

use crate::text::json_encode_test_support::encode;
use crate::text::json_encode_test_support::write_incremental;

/// Verifies output-byte quantity conversion rejects a document larger than a
/// narrow resource quantity before it is appended to the buffer.
#[test]
fn test_json_output_buffer_rejects_quantity_conversion_overflow() {
    let limits = JsonEncodeLimits::<JsonResource, u8>::builder()
        .output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, u8::MAX))
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);
    let value = "x".repeat(300);
    let error = encode(&value, &mut session).expect_err("output larger than u8 should reject quantity conversion");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
}

/// Verifies incremental output accounting rejects a narrow quantity while the
/// destination remains untouched.
#[test]
fn test_json_output_writer_rejects_quantity_conversion_overflow() {
    let limits = JsonEncodeLimits::<JsonResource, u8>::builder()
        .output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, u8::MAX))
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);
    let value = "x".repeat(300);
    let mut output = Vec::new();
    let error = write_incremental(&mut output, &value, &mut session)
        .expect_err("output larger than u8 should reject quantity conversion");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert!(!output.is_empty());
}

/// Verifies the output buffer rejects bytes beyond its configured budget.
#[test]
fn test_json_output_buffer_rejects_excess_output() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 3))
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);
    let error = encode(&"long", &mut session).expect_err("output should exceed the configured budget");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
}

/// Verifies successful buffered output records its complete byte count.
#[test]
fn test_json_output_buffer_accepts_complete_output() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 16))
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);
    let output = encode(&"ok", &mut session).expect("output within the bound should succeed");

    assert_eq!(output, br#""ok""#);
    assert_eq!(session.output_budget().expect("output budget").used(), 4);
}
