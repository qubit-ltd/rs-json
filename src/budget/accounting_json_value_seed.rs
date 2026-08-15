// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Incremental accounting deserialization into a JSON value tree.
// qubit-style: allow source-test-pair

use std::fmt;
use std::fmt::Debug;
use std::marker::PhantomData;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonContainerKind;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonValueTransaction;
use serde::Deserialize;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde::de::Error;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;

/// Serde seed that constructs a [`Value`] while accounting decoded resources.
///
/// Unlike lexical JSON admission, this seed observes values after a Serde
/// deserializer has decoded them. It is therefore suitable for default budget
/// enforcement inside a type's ordinary [`serde::Deserialize`]
/// implementation, where the original input bytes are unavailable.
pub struct AccountingJsonValueSeed<'transaction, 'budget, R, Q = usize>
where
    Q: ResourceQuantity,
{
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    depth: usize,
}

impl<'transaction, 'budget, R, Q>
    AccountingJsonValueSeed<'transaction, 'budget, R, Q>
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

impl<'de, R, Q> DeserializeSeed<'de> for AccountingJsonValueSeed<'_, '_, R, Q>
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

/// Visitor used by [`AccountingJsonValueSeed`].
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
    ) -> AccountingJsonValueSeed<'child, 'budget, R, Q> {
        AccountingJsonValueSeed {
            transaction: self.transaction,
            depth: self.depth.saturating_add(1),
        }
    }

    /// Creates a child seed that checks a container limit before decoding.
    fn prospective_child<'child>(
        &'child mut self,
        kind: JsonContainerKind,
        prospective: usize,
    ) -> AccountingJsonValueChildSeed<'child, 'budget, R, Q> {
        AccountingJsonValueChildSeed {
            transaction: self.transaction,
            depth: self.depth.saturating_add(1),
            kind,
            prospective,
        }
    }
}

/// A child seed that rejects excess container members before materialization.
struct AccountingJsonValueChildSeed<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    depth: usize,
    kind: JsonContainerKind,
    prospective: usize,
}

impl<'de, R, Q> DeserializeSeed<'de>
    for AccountingJsonValueChildSeed<'_, '_, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.transaction
            .check_container_count(self.kind, self.prospective)
            .map_err(D::Error::custom)?;
        AccountingJsonValueSeed {
            transaction: self.transaction,
            depth: self.depth,
        }
        .deserialize(deserializer)
    }
}

/// A map-key seed that checks the prospective entry before decoding its key.
struct AccountingJsonKeySeed<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    prospective: usize,
}

impl<'de, R, Q> DeserializeSeed<'de> for AccountingJsonKeySeed<'_, '_, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    type Value = String;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.transaction
            .check_container_count(JsonContainerKind::Map, self.prospective)
            .map_err(D::Error::custom)?;
        String::deserialize(deserializer)
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
        AccountingJsonValueSeed {
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
        AccountingJsonValueSeed {
            transaction: self.transaction,
            depth: self.depth,
        }
        .deserialize(deserializer)
    }

    fn visit_seq<A>(mut self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        loop {
            let next = values.len().checked_add(1).ok_or_else(|| {
                A::Error::custom("JSON sequence item count overflowed usize")
            })?;
            let Some(value) = sequence.next_element_seed(
                self.prospective_child(JsonContainerKind::Sequence, next),
            )?
            else {
                break;
            };
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
        let mut values = Map::new();
        let mut entries = 0_usize;
        loop {
            let next = entries.checked_add(1).ok_or_else(|| {
                A::Error::custom("JSON map entry count overflowed usize")
            })?;
            let Some(key) = map.next_key_seed(AccountingJsonKeySeed {
                transaction: self.transaction,
                prospective: next,
            })?
            else {
                break;
            };
            self.admit(JsonMeasurement::Key { bytes: key.len() })?;
            let value = map.next_value_seed(self.child())?;
            entries = next;
            values.insert(key, value);
        }
        self.admit(JsonMeasurement::Object {
            depth: self.depth,
            entries,
        })?;
        Ok(Value::Object(values))
    }
}
