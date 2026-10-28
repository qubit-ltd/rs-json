// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use crate::value::JsonValueRef;
use crate::value::SafeJsonValue;

#[test]
fn test_view_construction_and_copy_never_enter_copy_from() {
    let owner = SafeJsonValue::new(serde_json::json!({"a": [1, 2, 3]}));
    SafeJsonValue::take_copy_from_calls_for_test();

    let probe = SafeJsonValue::copy_from(owner.as_value());
    assert_eq!(SafeJsonValue::take_copy_from_calls_for_test(), 1);
    drop(probe);

    let view = JsonValueRef::new(owner.as_value());
    let copy = view;
    fn clone_value<T: Clone>(value: &T) -> T {
        value.clone()
    }
    let cloned = clone_value(&view);
    assert!(std::ptr::eq(copy.as_value(), owner.as_value()));
    assert!(std::ptr::eq(cloned.as_value(), owner.as_value()));
    assert_eq!(SafeJsonValue::take_copy_from_calls_for_test(), 0);
}
