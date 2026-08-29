// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests output accounting through the public encoder budget contract.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncodeError;
use qubit_json::encode::JsonEncoder;

/// Verifies output accounting returns a typed budget violation.
#[test]
fn test_json_output_accounting_reports_output_budget() {
    let session = JsonEncodeSession::from_limits(
        JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_output_bytes(1)
            .build(),
    );
    let error = JsonEncoder::new(session)
        .to_vec(&"ok")
        .expect_err("encoded string exceeds one output byte");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
}
