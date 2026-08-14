// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for safe strict JSON deserialization error metadata.

use qubit_json::text::JsonDeserializeError;
use qubit_json::text::JsonDeserializeErrorCategory;
use serde_json::from_slice;

/// Verifies that serde errors convert without retaining input details.
#[test]
fn test_json_deserialize_error_conversion_redacts_input_details() {
    let source = from_slice::<u64>(br#""TOP_SECRET""#)
        .expect_err("a JSON string cannot deserialize into u64");
    let error = JsonDeserializeError::from(source);

    assert_eq!(error.category(), JsonDeserializeErrorCategory::Data);
    assert_eq!(error.line(), 1);
    assert_eq!(error.column(), 12);
    assert!(!error.to_string().contains("TOP_SECRET"));
    assert!(!format!("{error:?}").contains("TOP_SECRET"));
}
