// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for strict JSON deserialization error categories.

use qubit_json::text::JsonDeserializeErrorCategory;

/// Verifies every category has a stable lowercase display name.
#[test]
fn test_json_deserialize_error_category_display_names() {
    let cases = [
        (JsonDeserializeErrorCategory::Data, "data"),
        (JsonDeserializeErrorCategory::Eof, "eof"),
        (JsonDeserializeErrorCategory::Io, "io"),
        (JsonDeserializeErrorCategory::Syntax, "syntax"),
    ];

    for (category, expected) in cases {
        assert_eq!(category.to_string(), expected);
    }
}
