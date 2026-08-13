// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_json::text::JsonEncodeError;
use serde_json::from_str;
use serde_json::Value;

/// Verifies that Serde failures retain the encoding error category.
#[test]
fn test_serialize_error_variant_is_distinct() {
    let error = from_str::<Value>("not-json")
        .expect_err("fixture must be invalid JSON");
    let error = JsonEncodeError::<(), usize>::Serialize(error);

    assert!(matches!(error, JsonEncodeError::Serialize(_)));
}
