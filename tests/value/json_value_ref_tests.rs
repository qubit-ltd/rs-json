// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_json::value::JsonValueRef;
use qubit_json::value::SafeJsonValue;
use serde_json::json;

#[test]
fn test_view_matches_owner_and_borrows_original() {
    let owner = SafeJsonValue::new(json!({"items": [1, "x", null]}));
    let view = JsonValueRef::new(owner.as_value());
    let copied = view;
    assert!(std::ptr::eq(copied.as_value(), owner.as_value()));
    assert_eq!(format!("{view:?}"), format!("{owner:?}"));
    assert_eq!(serde_json::to_vec(&view).unwrap(), serde_json::to_vec(&owner).unwrap());
    assert_eq!(view, copied);
}

#[test]
fn test_view_equality_uses_value_semantics() {
    let a = json!({"x": [1, 2]});
    let b = json!({"x": [1, 2]});
    let c = json!({"x": [2, 1]});
    let missing_key = json!({"y": [1, 2]});
    let shorter_array = json!({"x": [1]});
    assert_eq!(JsonValueRef::new(&a), JsonValueRef::new(&b));
    assert_ne!(JsonValueRef::new(&a), JsonValueRef::new(&c));
    assert_ne!(JsonValueRef::new(&a), JsonValueRef::new(&missing_key));
    assert_ne!(JsonValueRef::new(&a), JsonValueRef::new(&shorter_array));
    assert_ne!(JsonValueRef::new(&json!(null)), JsonValueRef::new(&json!(false)));
    assert_ne!(JsonValueRef::new(&json!(true)), JsonValueRef::new(&json!(0)));
    assert_ne!(JsonValueRef::new(&json!(1)), JsonValueRef::new(&json!("1")));
}

fn compare_lifetimes<'a, 'b>(a: JsonValueRef<'a>, b: JsonValueRef<'b>) -> bool {
    a == b
}

#[test]
fn test_view_has_reference_traits_and_cross_lifetime_equality() {
    fn require_traits<T: Copy + Clone + Send + Sync>() {}
    require_traits::<JsonValueRef<'static>>();
    let outer = json!([1]);
    let result = {
        let inner = json!([1]);
        compare_lifetimes(JsonValueRef::new(&outer), JsonValueRef::new(&inner))
    };
    assert!(result);
}
