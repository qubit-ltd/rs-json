// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Incremental budgeted deserialization into a JSON value tree.
// qubit-style: allow source-test-pair

use std::fmt;
use std::fmt::Debug;
use std::marker::PhantomData;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonValueTransaction;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde::de::Error;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;

/// Serde seed that constructs a [`Value`] while charging decoded resources.
///
/// Unlike lexical JSON admission, this seed observes values after a Serde
/// deserializer has decoded them. It is therefore suitable for default budget
/// enforcement inside a type's ordinary [`serde::Deserialize`]
/// implementation, where the original input bytes are unavailable.
pub struct BudgetedJsonValueSeed<'transaction, 'budget, R, Q = usize>
where
    Q: ResourceQuantity,
{
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    depth: usize,
}

impl<'transaction, 'budget, R, Q>
    BudgetedJsonValueSeed<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates a root seed using the supplied decoded-value transaction.
    pub fn new(
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    ) -> Self {
        Self {
            transaction,
            depth: 1,
        }
    }
}

impl<'de, R, Q> DeserializeSeed<'de> for BudgetedJsonValueSeed<'_, '_, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(BudgetedJsonValueVisitor {
            transaction: self.transaction,
            depth: self.depth,
            marker: PhantomData,
        })
    }
}

/// Visitor used by [`BudgetedJsonValueSeed`].
struct BudgetedJsonValueVisitor<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    depth: usize,
    marker: PhantomData<Q>,
}

impl<'transaction, 'budget, R, Q>
    BudgetedJsonValueVisitor<'transaction, 'budget, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// Stages one complete JSON measurement.
    fn admit<E>(&mut self, measurement: JsonMeasurement) -> Result<(), E>
    where
        E: Error,
    {
        self.transaction.try_admit(measurement).map_err(E::custom)
    }

    /// Charges a number's decoded text representation.
    fn enter_number<E>(&mut self, number: Number) -> Result<Value, E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Number {
            depth: self.depth,
            bytes: number.as_str().len(),
        })?;
        Ok(Value::Number(number))
    }

    /// Creates a child seed borrowing the same transaction.
    fn child<'child>(
        &'child mut self,
    ) -> BudgetedJsonValueSeed<'child, 'budget, R, Q> {
        BudgetedJsonValueSeed {
            transaction: self.transaction,
            depth: self.depth.saturating_add(1),
        }
    }
}

impl<'de, 'transaction, 'budget, R, Q> Visitor<'de>
    for BudgetedJsonValueVisitor<'transaction, 'budget, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a value representable as JSON")
    }

    fn visit_bool<E>(mut self, value: bool) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Boolean { depth: self.depth })?;
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(mut self, value: i64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.enter_number(Number::from(value))
    }

    #[inline(never)]
    fn visit_i128<E>(mut self, value: i128) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let number = Number::from_i128(value).expect(
            "serde_json arbitrary-precision support must represent i128",
        );
        self.enter_number(number)
    }

    fn visit_u64<E>(mut self, value: u64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.enter_number(Number::from(value))
    }

    #[inline(never)]
    fn visit_u128<E>(mut self, value: u128) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let number = Number::from_u128(value).expect(
            "serde_json arbitrary-precision support must represent u128",
        );
        self.enter_number(number)
    }

    fn visit_f64<E>(mut self, value: f64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let number = Number::from_f64(value).ok_or_else(|| {
            E::custom("non-finite float is not representable as JSON")
        })?;
        self.enter_number(number)
    }

    fn visit_str<E>(mut self, value: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::String {
            depth: self.depth,
            bytes: value.len(),
        })?;
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(mut self, value: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::String {
            depth: self.depth,
            bytes: value.len(),
        })?;
        Ok(Value::String(value))
    }

    fn visit_none<E>(mut self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
        Ok(Value::Null)
    }

    fn visit_unit<E>(mut self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        BudgetedJsonValueSeed {
            transaction: self.transaction,
            depth: self.depth,
        }
        .deserialize(deserializer)
    }

    fn visit_newtype_struct<D>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        BudgetedJsonValueSeed {
            transaction: self.transaction,
            depth: self.depth,
        }
        .deserialize(deserializer)
    }

    fn visit_seq<A>(mut self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0));
        while let Some(value) = sequence.next_element_seed(self.child())? {
            values.push(value);
        }
        self.admit(JsonMeasurement::Array {
            depth: self.depth,
            items: values.len(),
        })?;
        Ok(Value::Array(values))
    }

    fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::with_capacity(map.size_hint().unwrap_or(0));
        let mut entries = 0_usize;
        while let Some(key) = map.next_key::<String>()? {
            self.admit(JsonMeasurement::Key { bytes: key.len() })?;
            let value = map.next_value_seed(self.child())?;
            entries = entries.saturating_add(1);
            values.insert(key, value);
        }
        self.admit(JsonMeasurement::Object {
            depth: self.depth,
            entries,
        })?;
        Ok(Value::Object(values))
    }
}
