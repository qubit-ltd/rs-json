// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for bounded JSON output buffering.

use qubit_budget::ResourceLimit;
use qubit_json::JsonEncodeLimits;
use qubit_json::JsonEncodeSession;
use qubit_json::JsonResource;
use qubit_json::JsonSerdeError;
use qubit_json::encode_to_vec;

/// Verifies the output buffer rejects bytes beyond its configured budget.
#[test]
fn test_json_output_buffer_rejects_excess_output() {
    let limits = JsonEncodeLimits::empty()
        .with_output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 3));
    let mut session = JsonEncodeSession::owned(limits);
    let error = encode_to_vec(&"long", &mut session)
        .expect_err("output should exceed the configured budget");

    assert!(matches!(error, JsonSerdeError::Budget(_)));
}
