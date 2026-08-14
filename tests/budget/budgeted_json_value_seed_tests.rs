// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::marker::PhantomData;

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::BudgetedJsonValueSeed;
use serde::Deserializer as SerdeDeserializer;
use serde::de::DeserializeSeed;
use serde::de::Error as DeError;
use serde::de::Visitor;
use serde::de::value::F64Deserializer;
use serde::de::value::I64Deserializer;
use serde::de::value::I128Deserializer;
use serde::de::value::StrDeserializer;
use serde::de::value::StringDeserializer;
use serde::de::value::U64Deserializer;
use serde::de::value::U128Deserializer;
use serde::de::value::UnitDeserializer;
use serde::forward_to_deserialize_any;
use serde_json::Deserializer;
use serde_json::Error;
use serde_json::Number;
use serde_json::json;

/// Arbitrary-precision JSON number used to exercise serde_json's number
/// deserializer path.
const LARGE_NUMBER_TEXT: &str = "123456789012345678901234567890";

/// Deserializer that sends a seed directly to `visit_none`.
struct NoneDeserializer<E>(PhantomData<E>);

impl<E> NoneDeserializer<E> {
    /// Creates a deserializer that represents an absent optional value.
    const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<'de, E> SerdeDeserializer<'de> for NoneDeserializer<E>
where
    E: DeError,
{
    type Error = E;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_none()
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
        string bytes byte_buf option unit unit_struct newtype_struct seq
        tuple tuple_struct map struct enum identifier ignored_any
    }
}

/// Deserializer that sends a seed directly to `visit_some`.
struct SomeDeserializer<D>(D);

impl<'de, D> SerdeDeserializer<'de> for SomeDeserializer<D>
where
    D: SerdeDeserializer<'de>,
{
    type Error = D::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self.0)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 f32 f64 char str
        string bytes byte_buf option unit unit_struct newtype_struct seq
        tuple tuple_struct map struct enum identifier ignored_any
    }
}

/// Deserializer that sends a seed directly to `visit_newtype_struct`.
struct NewtypeDeserializer<D>(D);

impl<'de, D> SerdeDeserializer<'de> for NewtypeDeserializer<D>
where
    D: SerdeDeserializer<'de>,
{
    type Error = D::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self.0)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 f32 f64 char str
        string bytes byte_buf option unit unit_struct newtype_struct seq
        tuple tuple_struct map struct enum identifier ignored_any
    }
}

#[test]
fn budgeted_value_seed_rejects_decoded_nodes_incrementally() {
    let limits = JsonValueLimits::empty().with_structure_limits(
        StructureLimits::empty()
            .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 2)),
    );
    let mut budget = JsonValueBudget::new(limits);
    let mut deserializer = Deserializer::from_slice(br#"[1,2]"#);

    let error = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(&mut deserializer)
        .expect_err("the third decoded node should exceed the budget");

    assert!(error.to_string().contains("Nodes"));
    assert_eq!(budget.structure_budget().used_nodes(), 2);
}

#[test]
fn budgeted_value_seed_returns_the_admitted_value() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let mut deserializer = Deserializer::from_slice(br#"{"key":[true]}"#);

    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(&mut deserializer)
        .expect("the unconfigured budget should admit the value");

    assert_eq!(value, json!({"key": [true]}));
}

/// Verifies decoded scalar forms map through the seed without JSON text input.
#[test]
fn budgeted_value_seed_accepts_value_deserializer_scalar_forms() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(I64Deserializer::<Error>::new(-1))
        .expect("i64 deserializer should produce a JSON number");
    assert_eq!(value, json!(-1));

    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(F64Deserializer::<Error>::new(1.5))
        .expect("f64 deserializer should produce a JSON number");
    assert_eq!(value, json!(1.5));

    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(StringDeserializer::<Error>::new(String::from("text")))
        .expect("string deserializer should produce a JSON string");
    assert_eq!(value, json!("text"));

    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(UnitDeserializer::<Error>::new())
        .expect("unit deserializer should produce JSON null");
    assert_eq!(value, json!(null));

    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(I128Deserializer::<Error>::new(-1))
        .expect("i128 deserializer should produce a JSON number");
    assert_eq!(value, json!(-1));

    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(U64Deserializer::<Error>::new(1))
        .expect("u64 deserializer should produce a JSON number");
    assert_eq!(value, json!(1));

    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(U128Deserializer::<Error>::new(1))
        .expect("u128 deserializer should produce a JSON number");
    assert_eq!(value, json!(1));

    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(StrDeserializer::<Error>::new("borrowed"))
        .expect("str deserializer should produce a JSON string");
    assert_eq!(value, json!("borrowed"));

    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(NoneDeserializer::<Error>::new())
        .expect("none deserializer should produce JSON null");
    assert_eq!(value, json!(null));

    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(SomeDeserializer(I64Deserializer::<Error>::new(2)))
        .expect("some deserializer should delegate to its child seed");
    assert_eq!(value, json!(2));

    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(NewtypeDeserializer(I64Deserializer::<Error>::new(3)))
        .expect("newtype deserializer should delegate to its child seed");
    assert_eq!(value, json!(3));

    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let error = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(F64Deserializer::<Error>::new(f64::NAN))
        .expect_err("non-finite f64 should not be representable as JSON");
    assert!(error.to_string().contains("not representable"));

    let number = LARGE_NUMBER_TEXT
        .parse::<Number>()
        .expect("the number fixture must parse");
    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(number)
        .expect("serde_json number deserializer should produce a value");
    assert_eq!(value.to_string(), LARGE_NUMBER_TEXT);
}
