// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_json::value::SafeJsonValue;

#[test]
fn test_safe_value_bridge_rejects_decoded_duplicate_keys() {
    let error =
        serde_json::from_str::<SafeJsonValue>(r#"{"a":1,"\u0061":2}"#).expect_err("decoded duplicate keys must fail");
    assert!(error.is_data(), "duplicates must retain the data error category");
    assert!(
        error.to_string().contains("duplicate object key"),
        "duplicate diagnostic must stay stable"
    );
}

#[test]
fn test_safe_value_bridge_preserves_all_wide_object_entries() {
    let fields = (0..4096)
        .map(|i| format!("\"key-{i:04}\":{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let text = format!("{{{fields}}}");
    let owner: SafeJsonValue = serde_json::from_str(&text).expect("wide object must decode");
    let object = owner.as_value().as_object().expect("root must remain an object");
    assert_eq!(object.len(), 4096, "all distinct entries must be preserved");
    for i in 0..4096 {
        assert_eq!(object[&format!("key-{i:04}")].as_u64(), Some(i as u64));
    }
}

#[test]
fn test_safe_value_bridge_rejects_duplicates_at_both_object_positions() {
    for input in [r#"{"a":1,"b":2,"a":3}"#, r#"{"b":1,"a":2,"a":3}"#] {
        let error = serde_json::from_str::<SafeJsonValue>(input).expect_err("repeated key must fail");
        assert!(error.is_data(), "duplicates must retain the data error category");
        assert!(
            error.to_string().contains("duplicate object key"),
            "duplicate diagnostic must stay stable"
        );
    }
}

#[test]
fn test_safe_value_bridge_preserves_object_entry_order() {
    let input = r#"{"z":1,"a":2,"m":3}"#;
    let expected: serde_json::Value = serde_json::from_str(input).expect("reference object must decode");
    let owner: SafeJsonValue = serde_json::from_str(input).expect("unique object must decode");
    let object = owner.as_value().as_object().expect("root must remain an object");
    let expected = expected.as_object().expect("reference root must be an object");
    assert_eq!(
        object.keys().map(String::as_str).collect::<Vec<_>>(),
        expected.keys().map(String::as_str).collect::<Vec<_>>(),
    );
}
// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

mod migrated_owner {
    use qubit_json::value::SafeJsonValue;
    use serde::Deserialize;
    use serde_json::Value;
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
}
