// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_budget::json::JsonResource;
use qubit_json::tree::JsonTreeProcessError;

/// Verifies that visitor failures retain their domain payload.
#[test]
fn test_visitor_error_variant_retains_payload() {
    let error =
        JsonTreeProcessError::<JsonResource, usize, _>::Visitor("failed");

    assert!(matches!(error, JsonTreeProcessError::Visitor("failed")));
}
