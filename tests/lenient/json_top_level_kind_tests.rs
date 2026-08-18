// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public `JsonRootKind` type.

use std::str::FromStr;

use qubit_json::decode::JsonRootKind;
use serde_json::json;

/// Verifies that top level kind classifies values.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_top_level_kind_classifies_values() {
    assert_eq!(JsonRootKind::of(&json!({})), JsonRootKind::Object);
    assert_eq!(JsonRootKind::of(&json!([])), JsonRootKind::Array);
    assert_eq!(JsonRootKind::of(&json!(true)), JsonRootKind::Other);
}

/// Verifies that top level kind from matches of.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_top_level_kind_from_matches_of() {
    let value = json!([1, 2, 3]);
    assert_eq!(JsonRootKind::from(&value), JsonRootKind::of(&value));
}

/// Verifies that top level kind display uses lowercase names.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_top_level_kind_display_uses_lowercase_names() {
    assert_eq!(JsonRootKind::Object.to_string(), "object");
    assert_eq!(JsonRootKind::Array.to_string(), "array");
    assert_eq!(JsonRootKind::Other.to_string(), "other");
}

/// Verifies that top level kind from str.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_top_level_kind_from_str() {
    assert_eq!(
        JsonRootKind::from_str("object").expect("object must parse"),
        JsonRootKind::Object
    );
    assert_eq!(
        JsonRootKind::from_str("ARRAY").expect("ARRAY must parse without case sensitivity"),
        JsonRootKind::Array
    );
    assert_eq!(
        JsonRootKind::from_str("other").expect("other must parse"),
        JsonRootKind::Other
    );
    assert_eq!(JsonRootKind::from_str("dict"), Err("unknown JsonRootKind"),);
}
