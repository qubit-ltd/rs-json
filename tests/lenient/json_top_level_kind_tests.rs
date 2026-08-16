// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public `JsonTopLevelKind` type.

use std::str::FromStr;

use qubit_json::lenient::JsonTopLevelKind;
use serde_json::json;

/// Verifies that top level kind classifies values.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_top_level_kind_classifies_values() {
    assert_eq!(JsonTopLevelKind::of(&json!({})), JsonTopLevelKind::Object);
    assert_eq!(JsonTopLevelKind::of(&json!([])), JsonTopLevelKind::Array);
    assert_eq!(JsonTopLevelKind::of(&json!(true)), JsonTopLevelKind::Other);
}

/// Verifies that top level kind from matches of.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_top_level_kind_from_matches_of() {
    let value = json!([1, 2, 3]);
    assert_eq!(JsonTopLevelKind::from(&value), JsonTopLevelKind::of(&value));
}

/// Verifies that top level kind display uses lowercase names.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_top_level_kind_display_uses_lowercase_names() {
    assert_eq!(JsonTopLevelKind::Object.to_string(), "object");
    assert_eq!(JsonTopLevelKind::Array.to_string(), "array");
    assert_eq!(JsonTopLevelKind::Other.to_string(), "other");
}

/// Verifies that top level kind from str.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_top_level_kind_from_str() {
    assert_eq!(
        JsonTopLevelKind::from_str("object").expect("object must parse"),
        JsonTopLevelKind::Object
    );
    assert_eq!(
        JsonTopLevelKind::from_str("ARRAY")
            .expect("ARRAY must parse without case sensitivity"),
        JsonTopLevelKind::Array
    );
    assert_eq!(
        JsonTopLevelKind::from_str("other").expect("other must parse"),
        JsonTopLevelKind::Other
    );
    assert_eq!(
        JsonTopLevelKind::from_str("dict"),
        Err("unknown JsonTopLevelKind"),
    );
}
