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
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde::de::Error;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;

use super::JsonValueBudget;

/// Serde seed that constructs a [`Value`] while charging decoded resources.
///
/// Unlike lexical JSON admission, this seed observes values after a Serde
/// deserializer has decoded them. It is therefore suitable for default budget
/// enforcement inside a type's ordinary [`serde::Deserialize`]
/// implementation, where the original input bytes are unavailable.
pub struct BudgetedJsonValueSeed<'a, R, Q = usize>
where
    Q: ResourceQuantity,
{
    budget: &'a mut JsonValueBudget<R, Q>,
    depth: usize,
}

impl<'a, R, Q> BudgetedJsonValueSeed<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates a root seed using the supplied decoded-value budget.
    pub fn new(budget: &'a mut JsonValueBudget<R, Q>) -> Self {
        Self { budget, depth: 1 }
    }
}

impl<'de, R, Q> DeserializeSeed<'de> for BudgetedJsonValueSeed<'_, R, Q>
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
            budget: self.budget,
            depth: self.depth,
            marker: PhantomData,
        })
    }
}

/// Visitor used by [`BudgetedJsonValueSeed`].
struct BudgetedJsonValueVisitor<'a, R, Q>
where
    Q: ResourceQuantity,
{
    budget: &'a mut JsonValueBudget<R, Q>,
    depth: usize,
    marker: PhantomData<Q>,
}

impl<R, Q> BudgetedJsonValueVisitor<'_, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// Charges the current node.
    fn enter_node<E>(&mut self) -> Result<(), E>
    where
        E: Error,
    {
        self.budget.enter_node_usize(self.depth).map_err(E::custom)
    }

    /// Charges a number's decoded text representation.
    fn enter_number<E>(&mut self, number: Number) -> Result<Value, E>
    where
        E: Error,
    {
        self.enter_node()?;
        self.budget
            .consume_number_bytes_usize(number.as_str().len())
            .map_err(E::custom)?;
        Ok(Value::Number(number))
    }

    /// Creates a child seed borrowing the same budget.
    fn child<'child>(&'child mut self) -> BudgetedJsonValueSeed<'child, R, Q> {
        BudgetedJsonValueSeed {
            budget: self.budget,
            depth: self.depth.saturating_add(1),
        }
    }
}

impl<'de, R, Q> Visitor<'de> for BudgetedJsonValueVisitor<'_, R, Q>
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
        self.enter_node()?;
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(mut self, value: i64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.enter_number(Number::from(value))
    }

    fn visit_i128<E>(mut self, value: i128) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let number = Number::from_i128(value)
            .ok_or_else(|| E::custom("i128 value is not representable as JSON"))?;
        self.enter_number(number)
    }

    fn visit_u64<E>(mut self, value: u64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.enter_number(Number::from(value))
    }

    fn visit_u128<E>(mut self, value: u128) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let number = Number::from_u128(value)
            .ok_or_else(|| E::custom("u128 value is not representable as JSON"))?;
        self.enter_number(number)
    }

    fn visit_f64<E>(mut self, value: f64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let number = Number::from_f64(value)
            .ok_or_else(|| E::custom("non-finite float is not representable as JSON"))?;
        self.enter_number(number)
    }

    fn visit_str<E>(mut self, value: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.enter_node()?;
        self.budget
            .consume_string_bytes_usize(value.len())
            .map_err(E::custom)?;
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(mut self, value: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.enter_node()?;
        self.budget
            .consume_string_bytes_usize(value.len())
            .map_err(E::custom)?;
        Ok(Value::String(value))
    }

    fn visit_none<E>(mut self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.enter_node()?;
        Ok(Value::Null)
    }

    fn visit_unit<E>(mut self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.enter_node()?;
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        BudgetedJsonValueSeed {
            budget: self.budget,
            depth: self.depth,
        }
        .deserialize(deserializer)
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        BudgetedJsonValueSeed {
            budget: self.budget,
            depth: self.depth,
        }
        .deserialize(deserializer)
    }

    fn visit_seq<A>(mut self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        self.enter_node()?;
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0));
        while let Some(value) = sequence.next_element_seed(self.child())? {
            let count = values.len().saturating_add(1);
            self.budget
                .check_sequence_items_usize(count)
                .map_err(A::Error::custom)?;
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        self.enter_node()?;
        let mut values = Map::with_capacity(map.size_hint().unwrap_or(0));
        let mut count = 0_usize;
        while let Some(key) = map.next_key::<String>()? {
            count = count.saturating_add(1);
            self.budget
                .check_map_entries_usize(count)
                .map_err(A::Error::custom)?;
            self.budget
                .consume_key_bytes_usize(key.len())
                .map_err(A::Error::custom)?;
            let value = map.next_value_seed(self.child())?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
