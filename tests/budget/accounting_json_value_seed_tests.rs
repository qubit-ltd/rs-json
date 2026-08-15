// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Verifies incremental decoded-value accounting.

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::AccountingJsonValueSeed as BudgetedJsonValueSeed;
use serde::Deserializer as SerdeDeserializer;
use serde::de::DeserializeSeed;
use serde::de::Error as DeError;
use serde::de::Unexpected;
use serde::de::Visitor;
use serde::de::value;
use serde::forward_to_deserialize_any;
use serde_json::Deserializer;
use serde_json::json;

#[test]
fn budgeted_value_seed_rejects_decoded_nodes_incrementally() {
    let limits = JsonValueLimits::empty().with_structure_limits(
        StructureLimits::empty()
            .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 2)),
    );
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"[1,2]"#);

    let error = BudgetedJsonValueSeed::new(&mut transaction)
        .deserialize(&mut deserializer)
        .expect_err("the third decoded node should exceed the budget");

    assert!(error.to_string().contains("Nodes"));
    assert_eq!(budget.used_nodes(), Some(0));
}

#[test]
fn budgeted_value_seed_returns_the_admitted_value() {
    let mut budget = JsonValueLimits::empty().budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"{"key":[true]}"#);

    let value = BudgetedJsonValueSeed::new(&mut transaction)
        .deserialize(&mut deserializer)
        .expect("the unconfigured budget should admit the value");

    assert_eq!(value, json!({"key": [true]}));
}

/// Verifies duplicate keys count as separate decoded object entries.
#[test]
fn test_budgeted_value_seed_counts_duplicate_object_entries() {
    let limits = JsonValueLimits::empty().with_structure_limits(
        StructureLimits::empty().with_map_entries_limit(ResourceLimit::new(
            JsonResource::MapEntries,
            1,
        )),
    );
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let mut deserializer =
        Deserializer::from_slice(br#"{"key":null,"key":null}"#);

    let error = BudgetedJsonValueSeed::new(&mut transaction)
        .deserialize(&mut deserializer)
        .expect_err(
            "the duplicate second entry must exceed the object-entry limit",
        );

    assert!(error.to_string().contains("MapEntries"));
}

/// Exercises scalar and optional serde visitor branches that JSON text does
/// not necessarily dispatch to consistently across serde_json versions.
#[test]
fn test_budgeted_value_seed_accepts_all_scalar_deserializer_shapes() {
    let mut budget = JsonValueLimits::empty().budget();
    let mut transaction = budget.transaction();

    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(value::I64Deserializer::<value::Error>::new(-7))
            .expect("signed integers are JSON numbers"),
        json!(-7),
    );
    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(value::I128Deserializer::<value::Error>::new(1))
            .expect("i128 values fitting JSON are numbers"),
        json!(1),
    );
    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(value::U128Deserializer::<value::Error>::new(2))
            .expect("u128 values fitting JSON are numbers"),
        json!(2),
    );
    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(value::F64Deserializer::<value::Error>::new(3.5))
            .expect("finite floats are JSON numbers"),
        json!(3.5),
    );
    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(value::StrDeserializer::<value::Error>::new(
                "borrowed"
            ))
            .expect("borrowed strings are copied into JSON values"),
        json!("borrowed"),
    );
    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(value::StringDeserializer::<value::Error>::new(
                String::from("owned")
            ))
            .expect("owned strings are accepted"),
        json!("owned"),
    );
    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(value::UnitDeserializer::<value::Error>::new())
            .expect("unit maps to JSON null"),
        json!(null),
    );
}

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

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(
            value::BoolDeserializer::<value::Error>::new(true),
        )
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct
        enum identifier ignored_any
    }
}

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
fn test_budgeted_value_seed_handles_option_newtype_and_numeric_errors() {
    let mut budget = JsonValueLimits::empty().budget();
    let mut transaction = budget.transaction();
    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(value::I128Deserializer::<value::Error>::new(
                i128::MAX
            ))
            .expect("arbitrary precision supports i128"),
        json!(i128::MAX),
    );
    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(value::U128Deserializer::<value::Error>::new(
                u128::MAX
            ))
            .expect("arbitrary precision supports u128"),
        json!(u128::MAX),
    );
    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(I128DispatchDeserializer)
            .expect("explicit i128 visitor dispatch is supported"),
        json!(i128::MAX),
    );
    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(U128DispatchDeserializer)
            .expect("explicit u128 visitor dispatch is supported"),
        json!(u128::MAX),
    );
    assert!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(value::F64Deserializer::<value::Error>::new(f64::NAN))
            .is_err()
    );
    assert!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(ExpectingDeserializer)
            .is_err()
    );
    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(NoneDeserializer)
            .expect("visit_none maps to null"),
        json!(null),
    );
    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(SomeDeserializer)
            .expect("visit_some delegates to child seed"),
        json!(true),
    );
    assert_eq!(
        BudgetedJsonValueSeed::new(&mut transaction)
            .deserialize(NewtypeDeserializer)
            .expect("newtype visitor delegates to child seed"),
        json!(true),
    );
}

struct NewtypeDeserializer;

struct I128DispatchDeserializer;

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
        visitor.visit_newtype_struct(
            value::BoolDeserializer::<value::Error>::new(true),
        )
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}
