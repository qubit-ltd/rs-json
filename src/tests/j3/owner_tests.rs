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

struct CountingSink(usize);

impl std::fmt::Write for CountingSink {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.0 += text.len();
        Ok(())
    }
}

#[test]
fn test_owner_deep_lifecycle_in_small_stack() {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let mut raw = Value::Null;
            for _ in 0..32_768 {
                raw = Value::Array(vec![raw]);
            }
            let owner = SafeJsonValue::new(raw);
            let cloned = owner.clone();
            assert!(owner == cloned);
            let mut sink = CountingSink(0);
            std::fmt::write(&mut sink, format_args!("{owner:?}")).unwrap();
            assert!(sink.0 > 0);
            drop(cloned);
            drop(owner);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn test_owner_deep_comparison_detects_different_last_leaf() {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let mut left = Value::Null;
            let mut right = Value::Bool(true);
            for _ in 0..32_768 {
                left = Value::Array(vec![left]);
                right = Value::Array(vec![right]);
            }
            let left = SafeJsonValue::new(left);
            let right = SafeJsonValue::new(right);
            assert!(left != right);
            drop(left);
            drop(right);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn test_owner_handles_object_chain_and_wide_array() {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let mut object_tree = Value::Null;
            for index in 0..16_384 {
                let mut object = serde_json::Map::new();
                object.insert(format!("k{index}"), object_tree);
                object_tree = Value::Object(object);
            }
            let owner = SafeJsonValue::new(object_tree);
            let wide = SafeJsonValue::new(Value::Array(vec![Value::Null; 32_768]));
            drop(owner);
            drop(wide);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn test_owner_handles_ten_thousand_shallow_objects_in_a_wide_array() {
    let values = (0..10_000)
        .map(|index| Value::Object(serde_json::Map::from_iter([("index".to_owned(), Value::from(index))])))
        .collect();
    let owner = SafeJsonValue::new(Value::Array(values));
    let cloned = owner.clone();
    assert_eq!(owner.as_value().as_array().unwrap().len(), 10_000);
    assert!(owner == cloned);
}

#[test]
fn test_owner_debug_propagates_writer_failure() {
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
    let mut value = Value::Null;
    for _ in 0..1_024 {
        value = Value::Array(vec![value]);
    }
    let owner = SafeJsonValue::new(value);
    assert!(std::fmt::write(&mut LimitedSink(64), format_args!("{owner:?}")).is_err());
}

#[test]
fn test_owner_debug_and_equality_cover_all_json_value_kinds() {
    let value = SafeJsonValue::new(serde_json::json!({
        "array": [null, true, 7, "text"],
        "object": {"key": "value"}
    }));
    let cloned = value.clone();
    assert_eq!(value, cloned);
    let debug = format!("{value:?}");
    for expected in ["Null", "Bool(true)", "Number(7)", "String(\"text\")", "Object"] {
        assert!(
            debug.contains(expected),
            "debug output {debug:?} must contain {expected:?}"
        );
    }

    assert_ne!(
        SafeJsonValue::new(Value::String(String::from("left"))),
        SafeJsonValue::new(Value::String(String::from("right")))
    );
    assert_ne!(
        SafeJsonValue::new(serde_json::json!({"left": 1})),
        SafeJsonValue::new(serde_json::json!({"right": 1}))
    );
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
fn test_owner_serde_bridge_does_not_guess_private_map() {
    let text = r#"{"$serde_json::private::Number":"7"}"#;
    let owner: SafeJsonValue = serde_json::from_str(text).unwrap();
    assert!(owner.as_value().is_object());
    assert_eq!(
        owner
            .as_value()
            .get("$serde_json::private::Number")
            .and_then(Value::as_str),
        Some("7")
    );
}

#[test]
fn test_owner_bridge_rejects_decoded_duplicate_once() {
    let error = serde_json::from_str::<SafeJsonValue>(r#"{"a":0,"\u0061":1}"#).unwrap_err();
    assert!(error.is_data());
    assert!(error.line() >= 1);
    assert!(error.column() >= 1);
}

#[test]
fn test_owner_serde_bridge_preserves_finite_float_event() {
    use serde::de::value::F64Deserializer;

    let owner = SafeJsonValue::deserialize(F64Deserializer::<serde_json::Error>::new(1.25))
        .expect("finite f64 event must deserialize");
    assert_eq!(owner.as_value().as_f64(), Some(1.25));
    let serialized = serde_json::to_string(&owner).unwrap();
    assert_eq!(serialized, "1.25");
}

#[test]
fn test_owner_serde_bridge_accepts_unit_none_integer_and_owned_string_events() {
    use serde::de::value::I64Deserializer;
    use serde::de::value::StringDeserializer;
    use serde::de::value::UnitDeserializer;

    let unit = SafeJsonValue::deserialize(UnitDeserializer::<serde_json::Error>::new()).unwrap();
    assert!(unit.as_value().is_null());

    struct NoneDeserializer;

    impl<'de> serde::Deserializer<'de> for NoneDeserializer {
        type Error = serde_json::Error;

        fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: serde::de::Visitor<'de>,
        {
            visitor.visit_none()
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            bytes byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct
            map struct enum identifier ignored_any
        }
    }

    let none = SafeJsonValue::deserialize(NoneDeserializer).unwrap();
    assert!(none.as_value().is_null());

    let integer = SafeJsonValue::deserialize(I64Deserializer::<serde_json::Error>::new(-7)).unwrap();
    assert_eq!(integer.as_value().as_i64(), Some(-7));

    let string =
        SafeJsonValue::deserialize(StringDeserializer::<serde_json::Error>::new(String::from("owned"))).unwrap();
    assert_eq!(string.as_value().as_str(), Some("owned"));
}

#[test]
fn test_owner_serde_bridge_preserves_unsigned_number_and_nested_containers() {
    use serde::de::value::U64Deserializer;

    let number = SafeJsonValue::deserialize(U64Deserializer::<serde_json::Error>::new(u64::MAX)).unwrap();
    assert_eq!(number.as_value().as_u64(), Some(u64::MAX));

    let owner: SafeJsonValue = serde_json::from_str(r#"{"items":[1,{"enabled":true},null]}"#).unwrap();
    assert_eq!(
        owner.as_value(),
        &serde_json::json!({"items": [1, {"enabled": true}, null]})
    );
}

#[test]
fn test_owner_serde_bridge_propagates_non_finite_float_error() {
    use serde::de::value::F64Deserializer;

    let error = SafeJsonValue::deserialize(F64Deserializer::<serde_json::Error>::new(f64::INFINITY)).unwrap_err();
    assert!(error.is_data());
    assert!(error.to_string().contains("non-finite float"));
}

#[test]
fn test_owner_serde_bridge_treats_arbitrary_precision_private_map_as_object() {
    match serde_json::from_str::<SafeJsonValue>("1e400") {
        Ok(owner) => assert_eq!(
            owner
                .as_value()
                .get("$serde_json::private::Number")
                .and_then(Value::as_str),
            Some("1e+400")
        ),
        Err(error) => assert!(error.to_string().contains("number out of range")),
    }
}

#[test]
fn test_owner_serde_bridge_reports_truncated_input() {
    let error = serde_json::from_str::<SafeJsonValue>("[1,").unwrap_err();
    assert!(error.is_eof());
}

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
