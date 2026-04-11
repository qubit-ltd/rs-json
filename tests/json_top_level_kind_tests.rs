/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for the public `JsonTopLevelKind` type in `json_top_level_kind.rs`.
//!
//! Author: Haixing Hu

use serde_json::json;

use qubit_json::JsonTopLevelKind;

#[test]
fn test_top_level_kind_classifies_values() {
    assert_eq!(JsonTopLevelKind::of(&json!({})), JsonTopLevelKind::Object);
    assert_eq!(JsonTopLevelKind::of(&json!([])), JsonTopLevelKind::Array);
    assert_eq!(JsonTopLevelKind::of(&json!(true)), JsonTopLevelKind::Other);
}

#[test]
fn test_top_level_kind_from_matches_of() {
    let value = json!([1, 2, 3]);
    assert_eq!(JsonTopLevelKind::from(&value), JsonTopLevelKind::of(&value));
}

#[test]
fn test_top_level_kind_display_uses_lowercase_names() {
    assert_eq!(JsonTopLevelKind::Object.to_string(), "object");
    assert_eq!(JsonTopLevelKind::Array.to_string(), "array");
    assert_eq!(JsonTopLevelKind::Other.to_string(), "other");
}
