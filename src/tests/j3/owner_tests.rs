// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::cell::Cell;
use std::rc::Rc;

use serde::Deserialize;
use serde_json::Value;

use crate::lexical::JsonLexicalEvent;
use crate::lexical::JsonNumberModel;
use crate::value::SafeJsonValue;
use crate::value::internal::LexicalJsonValueBuilder;

struct CountingDeserializer<D> {
    inner: D,
    calls: Rc<Cell<usize>>,
}

impl<'de, D> serde::Deserializer<'de> for CountingDeserializer<D>
where
    D: serde::Deserializer<'de>,
{
    type Error = D::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.calls.set(self.calls.get() + 1);
        self.inner.deserialize_any(visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct
        map struct enum identifier ignored_any
    }
}

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
fn test_owner_deep_drop_is_safe_during_unwind() {
    let result = std::panic::catch_unwind(|| {
        let mut value = Value::Null;
        for _ in 0..32_768 {
            value = Value::Array(vec![value]);
        }
        let _owner = SafeJsonValue::new(value);
        panic!("exercise guarded owner drop during unwind");
    });
    assert!(result.is_err());
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
    assert!(result.is_err());
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
    assert!(result.is_err());
    SafeJsonValue::assert_guarded_insert_panic_consumed_for_test();
}

#[test]
fn test_builder_returns_guarded_value_and_stops_after_first_error() {
    let mut builder = LexicalJsonValueBuilder::new(JsonNumberModel::Integer64);
    assert!(builder.try_accept(JsonLexicalEvent::BeginObject { offset: 0 }).is_ok());
    assert!(
        builder
            .try_accept(JsonLexicalEvent::Key {
                token: br#""a""#,
                offset: 1
            })
            .is_ok()
    );
    assert!(builder.try_accept(JsonLexicalEvent::Null { offset: 5 }).is_ok());
    let duplicate = builder.try_accept(JsonLexicalEvent::Key {
        token: br#""a""#,
        offset: 9,
    });
    assert!(duplicate.is_err());
    assert!(builder.try_accept(JsonLexicalEvent::EndObject { offset: 12 }).is_err());
    assert!(builder.finish_guarded().is_err());
}

#[test]
fn test_builder_container_finalize_unwind_keeps_partial_tree_guarded() {
    for is_object in [false, true] {
        SafeJsonValue::panic_after_guarded_inserts_for_test(if is_object { 257 } else { 256 });
        let result = std::panic::catch_unwind(|| {
            let mut builder = LexicalJsonValueBuilder::new(JsonNumberModel::Integer64);
            if is_object {
                builder.try_accept(JsonLexicalEvent::BeginObject { offset: 0 }).unwrap();
                builder
                    .try_accept(JsonLexicalEvent::Key {
                        token: br#""deep""#,
                        offset: 1,
                    })
                    .unwrap();
            }
            for depth in 0..256 {
                builder
                    .try_accept(JsonLexicalEvent::BeginArray { offset: depth })
                    .unwrap();
            }
            builder.try_accept(JsonLexicalEvent::Null { offset: 256 }).unwrap();
            for depth in (0..256).rev() {
                builder
                    .try_accept(JsonLexicalEvent::EndArray { offset: depth })
                    .unwrap();
            }
            if is_object {
                builder.try_accept(JsonLexicalEvent::EndObject { offset: 257 }).unwrap();
            }
            drop(builder);
        });
        assert!(result.is_err());
        SafeJsonValue::assert_guarded_insert_panic_consumed_for_test();
    }
}

#[test]
fn test_owner_deserializes_through_one_single_use_deserializer() {
    let calls = Rc::new(Cell::new(0));
    let mut inner = serde_json::Deserializer::from_str(r#"{"items":[1,true]}"#);
    let deserializer = CountingDeserializer {
        inner: &mut inner,
        calls: calls.clone(),
    };
    let owner = SafeJsonValue::deserialize(deserializer).unwrap();
    assert!(owner.as_value().is_object());
    assert_eq!(calls.get(), 1);
}
