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
        error.to_string().contains("duplicate JSON object key"),
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
            error.to_string().contains("duplicate JSON object key"),
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

mod view_robustness {
    use std::io;
    use std::io::Write;

    use qubit_json::value::JsonValueRef;
    use qubit_json::value::SafeJsonValue;
    use serde::Serialize;
    use serde::Serializer;
    use serde::ser::Impossible;
    use serde_json::Value;

    fn owner_at_depth(depth: usize, leaf: i64) -> SafeJsonValue {
        let mut root = Value::from(leaf);
        for _ in 0..depth {
            root = Value::Array(vec![root]);
        }
        SafeJsonValue::new(root)
    }

    struct RejectAfter(usize);

    impl Write for RejectAfter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0 == 0 {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "view-test-sink"));
            }
            let written = self.0.min(bytes.len());
            self.0 -= written;
            Ok(written)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_deep_view_matches_owner_and_survives_write_error() {
        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(|| {
                let owner = owner_at_depth(4096, 7);
                let equal = owner_at_depth(4096, 7);
                let different = owner_at_depth(4096, 8);
                let view = JsonValueRef::new(owner.as_value());
                assert_eq!(view, JsonValueRef::new(equal.as_value()));
                assert_ne!(view, JsonValueRef::new(different.as_value()));
                assert_eq!(format!("{view:?}"), format!("{owner:?}"));
                assert_eq!(serde_json::to_vec(&view).unwrap(), serde_json::to_vec(&owner).unwrap());
                let error = serde_json::to_writer(RejectAfter(2048), &view).unwrap_err();
                assert_eq!(error.io_error_kind(), Some(io::ErrorKind::PermissionDenied));
                assert!(owner.as_value().is_array());
                assert!(!serde_json::to_vec(&view).unwrap().is_empty());
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn test_deep_mixed_view_keeps_owner_alive_after_copy_scope() {
        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(|| {
                let mut root = Value::from("leaf");
                for index in 0..2048 {
                    root = if index % 2 == 0 {
                        Value::Object(serde_json::Map::from_iter([("child".to_owned(), root)]))
                    } else {
                        Value::Array(vec![root])
                    };
                }
                let owner = SafeJsonValue::new(root);
                let view = JsonValueRef::new(owner.as_value());
                {
                    let copied = view;
                    assert!(std::ptr::eq(view.as_value(), copied.as_value()));
                    assert_eq!(view, copied);
                }
                assert!(!serde_json::to_vec(&view).unwrap().is_empty());
                assert!(owner.as_value().is_array() || owner.as_value().is_object());
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct SentinelError(u8);

    impl std::fmt::Display for SentinelError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "sentinel:{}", self.0)
        }
    }

    impl std::error::Error for SentinelError {}

    impl serde::ser::Error for SentinelError {
        fn custom<T: std::fmt::Display>(_: T) -> Self {
            Self(91)
        }
    }

    struct RejectSerializer;

    macro_rules! reject_scalar {
        ($($name:ident($ty:ty)),* $(,)?) => {
            $(fn $name(self, _: $ty) -> Result<(), SentinelError> {
                Err(SentinelError(37))
            })*
        };
    }

    impl Serializer for RejectSerializer {
        type Ok = ();
        type Error = SentinelError;
        type SerializeSeq = Impossible<(), SentinelError>;
        type SerializeTuple = Impossible<(), SentinelError>;
        type SerializeTupleStruct = Impossible<(), SentinelError>;
        type SerializeTupleVariant = Impossible<(), SentinelError>;
        type SerializeMap = Impossible<(), SentinelError>;
        type SerializeStruct = Impossible<(), SentinelError>;
        type SerializeStructVariant = Impossible<(), SentinelError>;

        reject_scalar!(
            serialize_bool(bool),
            serialize_i8(i8),
            serialize_i16(i16),
            serialize_i32(i32),
            serialize_i64(i64),
            serialize_u8(u8),
            serialize_u16(u16),
            serialize_u32(u32),
            serialize_u64(u64),
            serialize_f32(f32),
            serialize_f64(f64),
            serialize_char(char),
            serialize_str(&str),
            serialize_bytes(&[u8]),
        );

        fn serialize_none(self) -> Result<(), Self::Error> {
            Err(SentinelError(37))
        }
        fn serialize_unit(self) -> Result<(), Self::Error> {
            Err(SentinelError(37))
        }
        fn serialize_some<T: ?Sized + Serialize>(self, _: &T) -> Result<(), Self::Error> {
            Err(SentinelError(37))
        }
        fn serialize_unit_struct(self, _: &'static str) -> Result<(), Self::Error> {
            Err(SentinelError(37))
        }
        fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<(), Self::Error> {
            Err(SentinelError(37))
        }
        fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _: &'static str, _: &T) -> Result<(), Self::Error> {
            Err(SentinelError(37))
        }
        fn serialize_newtype_variant<T: ?Sized + Serialize>(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: &T,
        ) -> Result<(), Self::Error> {
            Err(SentinelError(37))
        }
        fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
            Err(SentinelError(37))
        }
        fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
            Err(SentinelError(37))
        }
        fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
            Err(SentinelError(37))
        }
        fn serialize_tuple_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeTupleVariant, Self::Error> {
            Err(SentinelError(37))
        }
        fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
            Err(SentinelError(37))
        }
        fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct, Self::Error> {
            Err(SentinelError(37))
        }
        fn serialize_struct_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeStructVariant, Self::Error> {
            Err(SentinelError(37))
        }
    }

    #[test]
    fn test_view_preserves_serializer_associated_error() {
        let owner = SafeJsonValue::new(serde_json::json!({"x": [1]}));
        let error = JsonValueRef::new(owner.as_value())
            .serialize(RejectSerializer)
            .unwrap_err();
        assert_eq!(error, SentinelError(37));
        assert!(owner.as_value().is_object());
    }
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
            Ok(owner) => {
                let number_text = owner
                    .as_value()
                    .get("$serde_json::private::Number")
                    .and_then(Value::as_str)
                    .expect("arbitrary-precision number text");
                assert!(number_text == "1e400" || number_text == "1e+400");
            }
            Err(error) => assert!(error.to_string().contains("number out of range")),
        }
    }

    #[test]
    fn test_owner_serde_bridge_numeric_semantics_follow_feature() {
        let arbitrary_precision = serde_json::from_str::<serde_json::Number>("1e400").is_ok();

        for (input, ordinary_serialization, arbitrary_number_text) in [
            ("1.25", "1.25", Some("1.25")),
            ("7", "7", None),
            ("1e2", "100.0", Some("1e2")),
            ("-0", "-0.0", None),
        ] {
            let owner: SafeJsonValue = serde_json::from_str(input).unwrap();
            if arbitrary_precision {
                if let Some(number_text) = arbitrary_number_text {
                    let actual = owner.as_value()["$serde_json::private::Number"]
                        .as_str()
                        .expect("arbitrary-precision number map");
                    assert!(
                        actual == number_text || (input == "1e2" && actual == "1e+2"),
                        "unexpected arbitrary-precision representation for {input}: {actual}"
                    );
                } else {
                    if input == "-0" {
                        let numeric_zero = owner.as_value().as_i64() == Some(0)
                            || owner.as_value()["$serde_json::private::Number"].as_str() == Some("-0");
                        assert!(numeric_zero, "negative zero should remain numerically zero");
                    } else {
                        assert_eq!(owner.as_value().as_i64(), Some(7));
                        assert_eq!(owner.as_value().to_string(), ordinary_serialization);
                    }
                }
            } else {
                assert_eq!(owner.as_value().to_string(), ordinary_serialization, "{input}");
                if input == "-0" {
                    assert_eq!(
                        owner.as_value().as_f64().unwrap().to_bits(),
                        (-0.0_f64).to_bits(),
                        "negative zero must retain its sign",
                    );
                }
            }
        }
    }

    #[test]
    fn test_owner_serde_bridge_nested_numeric_semantics_follow_feature() {
        let arbitrary_precision = serde_json::from_str::<serde_json::Number>("1e400").is_ok();
        let owner: SafeJsonValue = serde_json::from_str(r#"{"nested":[1.25,{"x":-0}]}"#).unwrap();

        if arbitrary_precision {
            assert_eq!(
                owner.as_value()["nested"][0]["$serde_json::private::Number"].as_str(),
                Some("1.25"),
            );
            let negative_zero = &owner.as_value()["nested"][1]["x"];
            assert!(
                negative_zero.as_i64() == Some(0)
                    || negative_zero["$serde_json::private::Number"].as_str() == Some("-0"),
                "arbitrary-precision Serde versions may expose negative zero as a number or private map"
            );
        } else {
            assert_eq!(owner.as_value(), &serde_json::json!({"nested": [1.25, {"x": -0.0}]}));
            assert_eq!(
                owner.as_value()["nested"][1]["x"].as_f64().unwrap().to_bits(),
                (-0.0_f64).to_bits(),
            );
        }
    }

    #[test]
    fn test_owner_decode_value_str_preserves_numeric_semantics_across_features() {
        let mut decoder = qubit_json::decode::JsonDecoder::unlimited();

        let scalar = decoder.decode_value_str("1.25").unwrap();
        assert_eq!(scalar.as_value().as_f64(), Some(1.25));

        let nested = decoder.decode_value_str(r#"{"nested":[1.25,1e2,-0]}"#).unwrap();
        assert_eq!(nested.as_value()["nested"][0].as_f64(), Some(1.25));
        assert_eq!(nested.as_value()["nested"][1].as_f64(), Some(100.0));
        assert_eq!(
            nested.as_value()["nested"][2].as_f64().unwrap().to_bits(),
            (-0.0_f64).to_bits(),
            "decoded negative zero must retain its sign",
        );
    }

    #[test]
    fn test_owner_serde_bridge_reports_truncated_input() {
        let error = serde_json::from_str::<SafeJsonValue>("[1,").unwrap_err();
        assert!(error.is_eof());
    }
}

#[test]
fn test_owner_comparison_and_debug_preserve_wide_tree_semantics() {
    let owner = SafeJsonValue::new(serde_json::json!([null, true, {"a": 7}]));
    assert_eq!(
        format!("{owner:?}"),
        "Array [Null, Bool(true), Object {\"a\": Number(Number(7))}]"
    );

    let width = 8192;
    let array = |changed: Option<usize>| {
        SafeJsonValue::new(serde_json::Value::Array(
            (0..width)
                .map(|index| serde_json::Value::from(if changed == Some(index) { -1 } else { index as i64 }))
                .collect(),
        ))
    };
    let baseline = array(None);
    assert_eq!(baseline, array(None));
    assert_ne!(baseline, array(Some(0)));
    assert_ne!(baseline, array(Some(width - 1)));

    let mut left = serde_json::Map::new();
    let mut right = serde_json::Map::new();
    for index in 0..width {
        left.insert(format!("key-{index:05}"), serde_json::Value::from(index as u64));
    }
    for index in (0..width).rev() {
        right.insert(format!("key-{index:05}"), serde_json::Value::from(index as u64));
    }
    let expected_object = SafeJsonValue::new(left.clone().into());
    assert_eq!(expected_object, SafeJsonValue::new(right.into()));
    left.remove("key-00000");
    assert_ne!(expected_object, SafeJsonValue::new(left.into()));
}
