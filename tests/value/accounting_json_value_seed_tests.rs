// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Verifies incremental decoded-value accounting through
//! `AccountingJsonValueSeed`.

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::AccountingJsonValueSeed;
use serde::Deserializer as SerdeDeserializer;
use serde::de::DeserializeSeed;
use serde::de::Error as DeError;
use serde::de::Unexpected;
use serde::de::Visitor;
use serde::de::value;
use serde::forward_to_deserialize_any;
use serde_json::Deserializer;
use serde_json::Value;
use serde_json::json;

#[test]
fn test_accounting_json_value_seed_rejects_decoded_nodes_incrementally() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, 2)))
        .build();
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"[1,2]"#);

    let error = AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(&mut deserializer)
        .expect_err("the third decoded node should exceed the budget");

    assert!(
        error.to_string().contains("Nodes"),
        "node limit error should name the exceeded resource"
    );
    assert_eq!(budget.used_nodes(), Some(0));
}

/// Verifies nested containers are admitted before their contents are decoded.
#[test]
fn test_accounting_json_value_seed_admits_containers_before_descending() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(
            StructureLimits::builder()
                .depth_limit(ResourceLimit::new(JsonResource::Depth, 1))
                .nodes_limit(ResourceLimit::new(JsonResource::Nodes, 3))
                .build(),
        )
        .build();
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"[[true]]"#);

    let error = AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(&mut deserializer)
        .expect_err("the nested array must exceed the depth limit before decoding its child");

    assert!(
        error.to_string().contains("Depth"),
        "depth limit error should name the exceeded resource"
    );
    assert_eq!(transaction.used_nodes(), Some(1));
}

#[test]
fn test_accounting_json_value_seed_returns_the_admitted_value() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder().build().budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"{"key":[true]}"#);

    let value = AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(&mut deserializer)
        .expect("the unconfigured budget should admit the value");

    assert_eq!(value, json!({"key": [true]}));
}

#[test]
fn test_accounting_json_value_seed_checks_number_display_bytes() {
    for maximum in [2_usize, 3] {
        let limits = JsonValueLimits::<JsonResource, usize>::builder()
            .max_number_bytes(maximum)
            .build();
        let mut budget = limits.budget();
        let mut transaction = budget.transaction();
        let result = AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::U64Deserializer::<value::Error>::new(100));
        if maximum == 3 {
            assert_eq!(result.unwrap(), json!(100));
            transaction.commit().unwrap();
        } else {
            let error = result.expect_err("three displayed digits must exceed two bytes");
            assert!(error.to_string().contains("NumberBytes"));
        }
    }
}

#[test]
fn test_accounting_json_value_seed_budgets_decoded_number_display() {
    let arbitrary_precision = serde_json::from_str::<Value>("1e400").is_ok();
    for maximum in [3_usize, 5] {
        let limits = JsonValueLimits::<JsonResource, usize>::builder()
            .max_number_bytes(maximum)
            .build();
        let mut budget = limits.budget();
        let mut transaction = budget.transaction();
        let mut deserializer = Deserializer::from_str("1e2");
        let result = AccountingJsonValueSeed::new(&mut transaction).deserialize(&mut deserializer);
        if arbitrary_precision {
            let value = result.expect("arbitrary-precision Serde exposes its private number map");
            let number_text = value["$serde_json::private::Number"]
                .as_str()
                .expect("arbitrary-precision number text");
            assert!(number_text == "1e2" || number_text == "1e+2");
            transaction.commit().unwrap();
        } else if maximum == 3 {
            let error = result.expect_err("the displayed number is longer than its source token");
            assert!(error.to_string().contains("NumberBytes"));
        } else {
            let value = result.expect("five displayed bytes should fit");
            assert_eq!(value.to_string(), "100.0");
            transaction.commit().unwrap();
        }
    }
}

/// Verifies duplicate keys count as separate decoded object entries.
#[test]
fn test_accounting_json_value_seed_counts_duplicate_object_entries() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(StructureLimits::builder().map_entries_limit(ResourceLimit::new(JsonResource::MapEntries, 1)))
        .build();
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"{"key":null,"key":null}"#);

    let error = AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(&mut deserializer)
        .expect_err("the duplicate second entry must exceed the object-entry limit");

    assert!(
        error.to_string().contains("MapEntries"),
        "map-entry limit error should name the exceeded resource"
    );
}

#[test]
fn test_accounting_seed_rejects_duplicate_keys_at_every_container_path() {
    for source in [
        r#"{"a":1,"a":2}"#,
        r#"{"outer":{"a":1,"a":2}}"#,
        r#"[{"a":1,"a":2}]"#,
        r#"{"a":1,"\u0061":2}"#,
    ] {
        let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
            .max_nodes(32)
            .build()
            .budget();
        {
            let mut transaction = budget.transaction();
            let mut parser = Deserializer::from_str(source);
            let error = AccountingJsonValueSeed::new(&mut transaction)
                .reject_duplicate_keys()
                .deserialize(&mut parser)
                .expect_err("duplicate keys must fail in every nested container");
            assert!(error.to_string().starts_with("duplicate JSON object key"));
        }
        assert_eq!(budget.used_nodes(), Some(0));
    }
}

#[test]
fn test_accounting_seed_rejects_duplicate_before_decoding_second_value() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder().build().budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#"{"sensitive-key":0,"sensitive-key":INVALID}"#);
    let error = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect_err("duplicate rejection must precede malformed second value");
    let message = error.to_string();
    assert!(message.starts_with("duplicate JSON object key"), "{message}");
    assert!(!message.contains("sensitive-key"), "{message}");
    assert!(!message.contains("INVALID"), "{message}");
}

#[test]
fn test_accounting_seed_default_keeps_last_value() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder().build().budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#"{"a":1,"a":2}"#);
    let value = AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(&mut parser)
        .expect("default seed remains compatible");
    parser.end().unwrap();
    assert_eq!(value, json!({"a": 2}));
    transaction.commit().unwrap();
}

#[test]
fn test_accounting_seed_duplicate_state_is_local_to_each_object() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder().build().budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#"[{"a":0},{"a":1}]"#);
    let value = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect("equal keys in distinct objects are allowed");
    assert_eq!(value, json!([{"a": 0}, {"a": 1}]));
    parser.end().unwrap();
    transaction.commit().unwrap();
}

#[test]
fn test_accounting_seed_checks_map_budget_before_duplicate_detection() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_map_entries(1)
        .build()
        .budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#"{"a":0,"a":1}"#);
    let error = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect_err("the second entry exceeds the map limit before duplicate detection");
    assert!(error.to_string().contains("MapEntries"), "{error}");
    assert!(!error.to_string().contains("duplicate JSON object key"));
}

#[test]
fn test_accounting_seed_checks_key_payload_budget_before_duplicate_detection() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_payload_bytes(1)
        .build()
        .budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#"{"a":null,"a":null}"#);
    let error = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect_err("the second key exceeds payload before duplicate detection");
    assert!(error.to_string().contains("PayloadBytes"), "{error}");
}

#[test]
fn test_accounting_seed_rejects_oversized_key_during_key_decode() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_key_bytes(1)
        .build()
        .budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#"{"aa":0,"aa":1}"#);
    let error = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect_err("the first oversized key must fail during key admission");
    assert!(error.to_string().contains("KeyBytes"), "{error}");
}

#[test]
fn test_accounting_seed_drops_failed_transaction_and_allows_retry() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(32)
        .build()
        .budget();
    {
        let mut transaction = budget.transaction();
        let mut parser = Deserializer::from_str(r#"{"a":0,"a":INVALID}"#);
        AccountingJsonValueSeed::new(&mut transaction)
            .reject_duplicate_keys()
            .deserialize(&mut parser)
            .expect_err("duplicate must fail");
    }
    assert_eq!(budget.used_nodes(), Some(0));
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#"{"ok":true}"#);
    let value = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect("a fresh transaction can retry");
    parser.end().unwrap();
    assert_eq!(value, json!({"ok": true}));
    transaction.commit().unwrap();
    assert_eq!(budget.used_nodes(), Some(2));
}

#[test]
fn test_accounting_seed_reject_option_preserves_private_number_marker_object() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder().build().budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#"{"$serde_json::private::Number":"1e400"}"#);
    let value = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect("map-shaped private marker events remain ordinary objects");
    assert_eq!(value, json!({"$serde_json::private::Number": "1e400"}));
    transaction.commit().unwrap();
}

#[test]
fn test_accounting_seed_reject_option_supports_u64_and_u8_quantities() {
    let mut budget = JsonValueLimits::<JsonResource, u64>::builder()
        .max_nodes(8)
        .build()
        .budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#"{"ok":true}"#);
    let value = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect("u64 quantities must admit ordinary objects");
    parser.end().unwrap();
    assert_eq!(value, json!({"ok": true}));
    transaction.commit().unwrap();
    assert_eq!(budget.used_nodes(), Some(2));

    let mut budget = JsonValueLimits::<JsonResource, u64>::builder().build().budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#"{"a":0,"a":1}"#);
    let error = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect_err("u64 quantities must retain duplicate rejection");
    assert!(error.to_string().starts_with("duplicate JSON object key"));

    let mut budget = JsonValueLimits::<JsonResource, u8>::builder().max_nodes(8).budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#"{"ok":true}"#);
    let value = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect("u8 quantities must admit ordinary objects");
    parser.end().unwrap();
    assert_eq!(value, json!({"ok": true}));
    transaction.commit().unwrap();
    assert_eq!(budget.used_nodes(), Some(2));

    let mut budget = JsonValueLimits::<JsonResource, u8>::builder().budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#"{"a":0,"a":1}"#);
    let error = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect_err("u8 quantities must retain duplicate rejection");
    assert!(error.to_string().starts_with("duplicate JSON object key"));
}

#[test]
fn test_accounting_seed_reject_option_preserves_sequence_item_admission() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_sequence_items(1)
        .build()
        .budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#"[{"a":0},{"a":1}]"#);
    let error = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect_err("the second array child exceeds SequenceItems before descent");
    assert!(error.to_string().contains("SequenceItems"), "{error}");
}

#[test]
fn test_accounting_seed_reject_option_preserves_depth_budget() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_depth(1)
        .build()
        .budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str("[[0]]");
    let error = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect_err("nested containers must still be rejected by Depth");
    assert!(error.to_string().contains("Depth"), "{error}");
}

#[test]
fn test_accounting_seed_reject_option_preserves_node_budget() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(2)
        .build()
        .budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str("[0,1]");
    let error = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect_err("the third node must still be rejected");
    assert!(error.to_string().contains("Nodes"), "{error}");
}

#[test]
fn test_accounting_seed_reject_option_preserves_string_byte_budget() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_string_bytes(1)
        .build()
        .budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str(r#""ab""#);
    let error = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect_err("the two-byte string must still exceed StringBytes");
    assert!(error.to_string().contains("StringBytes"), "{error}");
}

#[test]
fn test_accounting_seed_reject_option_preserves_number_byte_budget() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_number_bytes(1)
        .build()
        .budget();
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str("10");
    let error = AccountingJsonValueSeed::new(&mut transaction)
        .reject_duplicate_keys()
        .deserialize(&mut parser)
        .expect_err("the two-byte number must still exceed NumberBytes");
    assert!(error.to_string().contains("NumberBytes"), "{error}");
}

#[test]
fn test_accounting_seed_reject_option_preserves_numeric_event_contract() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder().build().budget();
    let mut transaction = budget.transaction();

    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .reject_duplicate_keys()
            .deserialize(value::I128Deserializer::<value::Error>::new(i64::MIN.into()))
            .expect("i64 minimum remains representable"),
        json!(i64::MIN),
    );
    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .reject_duplicate_keys()
            .deserialize(value::U128Deserializer::<value::Error>::new(u64::MAX.into()))
            .expect("u64 maximum remains representable"),
        json!(u64::MAX),
    );

    for (error, label) in [
        (
            AccountingJsonValueSeed::new(&mut transaction)
                .reject_duplicate_keys()
                .deserialize(value::I128Deserializer::<value::Error>::new(i64::MIN as i128 - 1))
                .expect_err("i128 below i64 minimum must be rejected"),
            "i128 below i64 minimum",
        ),
        (
            AccountingJsonValueSeed::new(&mut transaction)
                .reject_duplicate_keys()
                .deserialize(value::I128Deserializer::<value::Error>::new(u64::MAX as i128 + 1))
                .expect_err("i128 above u64 maximum must be rejected"),
            "i128 above u64 maximum",
        ),
        (
            AccountingJsonValueSeed::new(&mut transaction)
                .reject_duplicate_keys()
                .deserialize(value::U128Deserializer::<value::Error>::new(u128::MAX))
                .expect_err("u128 above u64 maximum must be rejected"),
            "u128 above u64 maximum",
        ),
    ] {
        assert!(
            error.to_string().starts_with("JSON integer is outside"),
            "{label}: {error}"
        );
    }

    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let error = AccountingJsonValueSeed::new(&mut transaction)
            .reject_duplicate_keys()
            .deserialize(value::F64Deserializer::<value::Error>::new(value))
            .expect_err("non-finite float must not become a JSON number");
        assert!(error.to_string().starts_with("non-finite float"), "{error}");
    }
}

#[test]
fn test_accounting_seed_discards_transaction_after_document_end_failure() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(10)
        .build()
        .budget();
    {
        let mut transaction = budget.transaction();
        let mut parser = Deserializer::from_str(r#"{"a":0} true"#);
        let value = AccountingJsonValueSeed::new(&mut transaction)
            .reject_duplicate_keys()
            .deserialize(&mut parser)
            .expect("the seed consumes the first value");
        assert_eq!(value, json!({"a": 0}));
        assert!(parser.end().is_err(), "the trailing value must fail end-of-input");
    }
    assert_eq!(budget.used_nodes(), Some(0));
}

/// Exercises scalar and optional serde visitor branches that JSON text does
/// not necessarily dispatch to consistently across serde_json versions.
#[test]
fn test_accounting_json_value_seed_accepts_all_scalar_deserializer_shapes() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder().build().budget();
    let mut transaction = budget.transaction();

    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::I64Deserializer::<value::Error>::new(-7))
            .expect("signed integers are JSON numbers"),
        json!(-7),
    );
    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::I128Deserializer::<value::Error>::new(1))
            .expect("i128 values fitting JSON are numbers"),
        json!(1),
    );
    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::U128Deserializer::<value::Error>::new(2))
            .expect("u128 values fitting JSON are numbers"),
        json!(2),
    );
    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::F64Deserializer::<value::Error>::new(3.5))
            .expect("finite floats are JSON numbers"),
        json!(3.5),
    );
    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::StrDeserializer::<value::Error>::new("borrowed"))
            .expect("borrowed strings are copied into JSON values"),
        json!("borrowed"),
    );
    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::StringDeserializer::<value::Error>::new(String::from("owned")))
            .expect("owned strings are accepted"),
        json!("owned"),
    );
    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::UnitDeserializer::<value::Error>::new())
            .expect("unit maps to JSON null"),
        json!(null),
    );
}

/// Verifies scalar visitor paths also instantiate for byte-sized quantities.
#[test]
fn test_accounting_json_value_seed_supports_u8_resource_quantities() {
    let mut budget = JsonValueLimits::<JsonResource, u8>::builder().budget();
    let mut transaction = budget.transaction();

    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::StringDeserializer::<value::Error>::new(String::from("owned-u8"),))
            .expect("owned strings should be admitted"),
        json!("owned-u8"),
    );
    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::UnitDeserializer::<value::Error>::new())
            .expect("unit should become null"),
        json!(null),
    );
}

/// Dispatches the option visitor's `visit_none` branch and boolean newtype
/// data.
struct NoneDeserializer;

impl<'de> SerdeDeserializer<'de> for NoneDeserializer {
    type Error = value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_none()
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_none()
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(value::BoolDeserializer::<value::Error>::new(true))
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct
        enum identifier ignored_any
    }
}

/// Dispatches a boolean child through the option visitor's `visit_some` branch.
struct SomeDeserializer;

impl<'de> SerdeDeserializer<'de> for SomeDeserializer {
    type Error = value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(value::BoolDeserializer::<value::Error>::new(true))
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple tuple_struct
        map struct enum identifier ignored_any
    }
}

/// Covers explicit serde visitor branches for option/newtype delegation and
/// non-representable numeric values.
#[test]
fn test_accounting_json_value_seed_handles_option_newtype_and_numeric_errors() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder().build().budget();
    let mut transaction = budget.transaction();
    assert!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::I128Deserializer::<value::Error>::new(i128::MAX))
            .is_err(),
        "i128::MAX should exceed the supported JSON integer range"
    );
    assert!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::U128Deserializer::<value::Error>::new(u128::MAX))
            .is_err(),
        "u128::MAX should exceed the supported JSON integer range"
    );
    assert!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(I128DispatchDeserializer)
            .is_err(),
        "the custom i128 visitor branch should reject an out-of-range value"
    );
    assert!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(U128DispatchDeserializer)
            .is_err(),
        "the custom u128 visitor branch should reject an out-of-range value"
    );
    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::I128Deserializer::<value::Error>::new(i64::MIN.into()))
            .expect("i64 minimum must remain representable"),
        json!(i64::MIN),
    );
    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::U128Deserializer::<value::Error>::new(u64::MAX.into()))
            .expect("u64 maximum must remain representable"),
        json!(u64::MAX),
    );
    assert!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(value::F64Deserializer::<value::Error>::new(f64::NAN))
            .is_err(),
        "non-finite floats must not become JSON numbers"
    );
    assert!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(ExpectingDeserializer)
            .is_err(),
        "unsupported visitor types should return a serde error"
    );
    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(NoneDeserializer)
            .expect("visit_none maps to null"),
        json!(null),
    );
    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(SomeDeserializer)
            .expect("visit_some delegates to child seed"),
        json!(true),
    );
    assert_eq!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(NewtypeDeserializer)
            .expect("newtype visitor delegates to child seed"),
        json!(true),
    );
}

#[test]
fn test_accounting_seed_discards_partial_usage_after_syntax_failure() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(10)
        .build()
        .budget();
    {
        let mut transaction = budget.transaction();
        let mut parser = Deserializer::from_str("[0,");
        let error = AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(&mut parser)
            .expect_err("the incomplete array must fail on syntax");
        assert!(error.is_eof(), "failure must not be a budget rejection");
        assert_eq!(transaction.used_nodes(), Some(2));
    }
    assert_eq!(budget.used_nodes(), Some(0));
    let mut transaction = budget.transaction();
    let mut parser = Deserializer::from_str("[0]");
    let value = AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(&mut parser)
        .unwrap();
    parser.end().unwrap();
    assert_eq!(value, json!([0]));
    transaction.commit().unwrap();
    assert_eq!(budget.used_nodes(), Some(2));
}

#[test]
fn test_accounting_seed_discards_usage_after_document_end_failure() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(10)
        .build()
        .budget();
    {
        let mut transaction = budget.transaction();
        let mut parser = Deserializer::from_str("[0] true");
        let value = AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(&mut parser)
            .unwrap();
        assert_eq!(value, json!([0]));
        assert!(parser.end().is_err(), "the complete document contains trailing data");
        assert_eq!(transaction.used_nodes(), Some(2));
    }
    assert_eq!(budget.used_nodes(), Some(0));
}

/// Dispatches a boolean through the visitor's newtype delegation branch.
struct NewtypeDeserializer;

/// Delivers an out-of-range signed integer through `visit_i128`.
struct I128DispatchDeserializer;

/// Delivers an out-of-range unsigned integer through `visit_u128`.
struct U128DispatchDeserializer;

impl<'de> SerdeDeserializer<'de> for I128DispatchDeserializer {
    type Error = value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i128(i128::MAX)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de> SerdeDeserializer<'de> for U128DispatchDeserializer {
    type Error = value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u128(u128::MAX)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

/// Supplies unit to exercise the visitor's unexpected-type error path.
struct ExpectingDeserializer;

impl<'de> SerdeDeserializer<'de> for ExpectingDeserializer {
    type Error = value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(value::Error::invalid_type(Unexpected::Unit, &visitor))
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de> SerdeDeserializer<'de> for NewtypeDeserializer {
    type Error = value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(value::BoolDeserializer::<value::Error>::new(true))
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}
