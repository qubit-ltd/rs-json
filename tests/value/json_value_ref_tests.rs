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

#[test]
fn test_view_comparison_and_debug_preserve_wide_tree_semantics() {
    let value = json!([null, true, {"a": 7}]);
    let view = JsonValueRef::new(&value);
    assert_eq!(
        format!("{view:?}"),
        "Array [Null, Bool(true), Object {\"a\": Number(Number(7))}]"
    );

    let width = 8192;
    let left = serde_json::Value::Array((0..width).map(|index| json!(index)).collect());
    let equal = serde_json::Value::Array((0..width).map(|index| json!(index)).collect());
    let different = serde_json::Value::Array(
        (0..width)
            .map(|index| json!(if index == width - 1 { -1 } else { index as i64 }))
            .collect(),
    );
    assert_eq!(JsonValueRef::new(&left), JsonValueRef::new(&equal));
    assert_ne!(JsonValueRef::new(&left), JsonValueRef::new(&different));

    let mut first = serde_json::Map::new();
    let mut second = serde_json::Map::new();
    for index in 0..width {
        first.insert(format!("key-{index:05}"), json!(index));
    }
    for index in (0..width).rev() {
        second.insert(format!("key-{index:05}"), json!(index));
    }
    let first = serde_json::Value::Object(first);
    let second = serde_json::Value::Object(second);
    assert_eq!(JsonValueRef::new(&first), JsonValueRef::new(&second));
}

#[test]
fn test_view_debug_propagates_writer_failure() {
    struct LimitedSink(usize);

    impl std::fmt::Write for LimitedSink {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            if text.len() > self.0 {
                return Err(std::fmt::Error);
            }
            self.0 -= text.len();
            Ok(())
        }
    }

    let value = json!([0, 1, 2, 3, 4, 5]);
    let view = JsonValueRef::new(&value);
    assert!(std::fmt::write(&mut LimitedSink(8), format_args!("{view:?}")).is_err());
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
