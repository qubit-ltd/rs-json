// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use serde::Deserialize;
use serde_json::Value;

use crate::lexical::JsonLexicalEvent;
use crate::lexical::JsonNumberModel;
use crate::value::SafeJsonValue;
use crate::value::internal::LexicalJsonValueBuilder;
use crate::value::internal::SafeValueBuilder;

#[test]
fn test_owner_drop_uses_allocation_free_fallback_when_reserve_fails() {
    let mut value = Value::Null;
    for _ in 0..32 {
        value = Value::Array(vec![value]);
    }
    let owner = SafeJsonValue::new(value);
    SafeJsonValue::fail_drop_reserve_after_for_test(0);
    drop(owner);
    SafeJsonValue::assert_drop_reserve_failure_consumed_for_test();

    for (depth, successful_reserves) in [(64, 64), (256, 256)] {
        let mut value = Value::Null;
        for _ in 0..depth {
            value = Value::Array(vec![value]);
        }
        let owner = SafeJsonValue::new(value);
        SafeJsonValue::fail_drop_reserve_after_for_test(successful_reserves);
        drop(owner);
        SafeJsonValue::assert_drop_reserve_failure_consumed_for_test();
    }
}

#[test]
fn test_owner_drop_fallback_mixed_array_with_pending_siblings() {
    for successful_reserves in [0, 1, 2] {
        let owner = SafeJsonValue::new(serde_json::json!([
            {"first": ["a", null, {}]},
            {"second": [[], "b"]},
            {"third": {"nested": "c"}}
        ]));
        SafeJsonValue::fail_drop_reserve_after_for_test(successful_reserves);
        drop(owner);
        SafeJsonValue::assert_drop_reserve_failure_consumed_for_test();
    }
}

#[test]
fn test_owner_drop_fallback_mixed_object_with_pending_siblings() {
    for successful_reserves in [0, 1, 2] {
        let owner = SafeJsonValue::new(serde_json::json!({
            "a": ["a", null, {}],
            "b": {"nested": [[], "b"]},
            "c": ["c", {"leaf": null}]
        }));
        SafeJsonValue::fail_drop_reserve_after_for_test(successful_reserves);
        drop(owner);
        SafeJsonValue::assert_drop_reserve_failure_consumed_for_test();
    }
}

#[test]
fn test_owner_copy_from_unwind_drops_partial_object_iteratively() {
    let mut source = Value::Null;
    for index in 0..128 {
        let mut object = serde_json::Map::new();
        object.insert(format!("k{index}"), source);
        source = Value::Object(object);
    }
    let source = SafeJsonValue::new(source);
    SafeJsonValue::panic_after_guarded_inserts_for_test(64);
    let result = std::panic::catch_unwind(|| SafeJsonValue::copy_from(source.as_value()));
    assert!(
        result.is_err(),
        "copy_from should unwind after the injected object insertion panic"
    );
    SafeJsonValue::assert_guarded_insert_panic_consumed_for_test();
    drop(source);
}

#[test]
fn test_owner_copy_from_unwind_drops_partial_array_iteratively() {
    let mut source = Value::Null;
    for _ in 0..128 {
        source = Value::Array(vec![source]);
    }
    let source = SafeJsonValue::new(source);
    SafeJsonValue::panic_after_guarded_inserts_for_test(64);
    let result = std::panic::catch_unwind(|| SafeJsonValue::copy_from(source.as_value()));
    assert!(
        result.is_err(),
        "copy_from should unwind after the injected array insertion panic"
    );
    SafeJsonValue::assert_guarded_insert_panic_consumed_for_test();
    drop(source);
}

#[test]
fn test_owner_serde_unwind_drops_partial_array_iteratively() {
    let text = format!("{}0{}", "[".repeat(128), "]".repeat(128));
    SafeJsonValue::panic_after_guarded_inserts_for_test(64);
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    deserializer.disable_recursion_limit();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        SafeJsonValue::deserialize(&mut deserializer)
    }));
    assert!(
        result.is_err(),
        "deserialization should unwind after the injected insertion panic"
    );
    SafeJsonValue::assert_guarded_insert_panic_consumed_for_test();
}

#[test]
fn test_array_accumulator_unwind_drops_inserted_deep_child_iteratively() {
    let mut child = Value::Null;
    for _ in 0..128 {
        child = Value::Array(vec![child]);
    }

    SafeJsonValue::panic_after_guarded_inserts_for_test(0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut values = SafeValueBuilder::array_with_capacity(1);
        values.push(SafeJsonValue::new(child));
    }));
    assert!(result.is_err(), "the guarded array insertion hook must fire");
    SafeJsonValue::assert_guarded_insert_panic_consumed_for_test();
}

#[test]
fn test_builder_returns_guarded_value_and_stops_after_first_error() {
    let mut builder = LexicalJsonValueBuilder::new(JsonNumberModel::Integer64);
    assert!(
        builder.try_accept(JsonLexicalEvent::BeginObject { offset: 0 }).is_ok(),
        "the builder should accept the object start"
    );
    assert!(
        builder
            .try_accept(JsonLexicalEvent::Key {
                token: br#""a""#,
                offset: 1
            })
            .is_ok(),
        "the builder should accept the first object key"
    );
    assert!(
        builder.try_accept(JsonLexicalEvent::Null { offset: 5 }).is_ok(),
        "the builder should accept the first object value"
    );
    let duplicate = builder.try_accept(JsonLexicalEvent::Key {
        token: br#""a""#,
        offset: 9,
    });
    assert!(duplicate.is_err(), "the builder should reject a duplicate key");
    assert!(
        builder.try_accept(JsonLexicalEvent::EndObject { offset: 12 }).is_err(),
        "the builder should remain failed after rejecting a duplicate key"
    );
    assert!(
        builder.finish_guarded().is_err(),
        "a failed builder should not produce a guarded value"
    );
}

#[test]
fn test_builder_container_finalize_unwind_keeps_partial_tree_guarded() {
    for is_object in [false, true] {
        SafeJsonValue::panic_after_guarded_inserts_for_test(if is_object { 257 } else { 256 });
        let result = std::panic::catch_unwind(|| {
            let mut builder = LexicalJsonValueBuilder::new(JsonNumberModel::Integer64);
            if is_object {
                builder
                    .try_accept(JsonLexicalEvent::BeginObject { offset: 0 })
                    .expect("the builder should accept the object start");
                builder
                    .try_accept(JsonLexicalEvent::Key {
                        token: br#""deep""#,
                        offset: 1,
                    })
                    .expect("the builder should accept the object key");
            }
            for depth in 0..256 {
                builder
                    .try_accept(JsonLexicalEvent::BeginArray { offset: depth })
                    .expect("the builder should accept each nested array start");
            }
            builder
                .try_accept(JsonLexicalEvent::Null { offset: 256 })
                .expect("the builder should accept the leaf value");
            for depth in (0..256).rev() {
                builder
                    .try_accept(JsonLexicalEvent::EndArray { offset: depth })
                    .expect("the builder should accept each nested array end");
            }
            if is_object {
                builder
                    .try_accept(JsonLexicalEvent::EndObject { offset: 257 })
                    .expect("the builder should accept the object end");
            }
            drop(builder);
        });
        assert!(
            result.is_err(),
            "container finalization should unwind after the injected insertion panic"
        );
        SafeJsonValue::assert_guarded_insert_panic_consumed_for_test();
    }
}
