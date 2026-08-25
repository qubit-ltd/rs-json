// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serde visitor that builds and accounts a JSON value.

use std::fmt;
use std::fmt::Debug;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonContainerKind;
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

use super::JsonKeySeed;
use super::JsonValueChildSeed;
use super::json_number_lexeme_length;

/// Visitor used to construct and account one JSON value.
pub(in crate::value) struct JsonValueVisitor<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Transaction receiving measurements for the value under construction.
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,

    /// Root-inclusive depth of the value currently being visited.
    depth: usize,
}

impl<'transaction, 'budget, R, Q> JsonValueVisitor<'transaction, 'budget, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// Creates a visitor that accounts a value at `depth`.
    pub(in crate::value) fn new(
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
        depth: usize,
    ) -> Self {
        Self { transaction, depth }
    }

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
            bytes: json_number_lexeme_length(&number),
        })?;
        Ok(Value::Number(number))
    }

    /// Creates a child visitor borrowing the same transaction.
    fn child<'child>(&'child mut self) -> JsonValueVisitor<'child, 'budget, R, Q> {
        JsonValueVisitor::new(self.transaction, self.depth.saturating_add(1))
    }

    /// Creates a child seed that checks a container limit before decoding.
    fn prospective_child<'child>(
        &'child mut self,
        kind: JsonContainerKind,
        prospective: usize,
    ) -> JsonValueChildSeed<'child, 'budget, R, Q> {
        JsonValueChildSeed {
            transaction: self.transaction,
            depth: self.depth.saturating_add(1),
            kind,
            prospective,
        }
    }
}

impl<'de, R, Q> DeserializeSeed<'de> for JsonValueVisitor<'_, '_, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    type Value = Value;

    /// Starts an `deserialize_any` visit for the requested JSON value.
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de, 'transaction, 'budget, R, Q> Visitor<'de> for JsonValueVisitor<'transaction, 'budget, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    type Value = Value;

    /// Describes the JSON value shape accepted by this visitor.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a value representable as JSON")
    }

    /// Accounts and constructs a JSON boolean.
    fn visit_bool<E>(mut self, value: bool) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Boolean { depth: self.depth })?;
        Ok(Value::Bool(value))
    }

    /// Accounts and constructs a signed JSON integer.
    fn visit_i64<E>(mut self, value: i64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.enter_number(Number::from(value))
    }

    /// Rejects unsupported wide signed integers or accounts the representable
    /// value.
    #[inline(never)]
    fn visit_i128<E>(mut self, value: i128) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let number =
            Number::from_i128(value).ok_or_else(|| E::custom("JSON integer is outside the supported 64-bit range"))?;
        self.enter_number(number)
    }

    /// Accounts and constructs an unsigned JSON integer.
    fn visit_u64<E>(mut self, value: u64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.enter_number(Number::from(value))
    }

    /// Rejects unsupported wide unsigned integers or accounts the representable
    /// value.
    #[inline(never)]
    fn visit_u128<E>(mut self, value: u128) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let number =
            Number::from_u128(value).ok_or_else(|| E::custom("JSON integer is outside the supported 64-bit range"))?;
        self.enter_number(number)
    }

    /// Rejects non-finite floats or accounts the representable JSON number.
    fn visit_f64<E>(mut self, value: f64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let number =
            Number::from_f64(value).ok_or_else(|| E::custom("non-finite float is not representable as JSON"))?;
        self.enter_number(number)
    }

    /// Accounts and copies a borrowed JSON string.
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

    /// Accounts and adopts an owned JSON string.
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

    /// Accounts a missing optional value as JSON null.
    fn visit_none<E>(mut self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
        Ok(Value::Null)
    }

    /// Accounts a unit value as JSON null.
    fn visit_unit<E>(mut self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
        Ok(Value::Null)
    }

    /// Delegates an optional value to the same JSON visitor.
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    /// Delegates a newtype payload to the same JSON visitor.
    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    /// Accounts and materializes a JSON array from sequence events.
    fn visit_seq<A>(mut self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        self.transaction
            .try_enter_container(JsonContainerKind::Sequence, self.depth)
            .map_err(A::Error::custom)?;
        let mut values = Vec::new();
        loop {
            let next = values
                .len()
                .checked_add(1)
                .ok_or_else(|| A::Error::custom("JSON sequence item count overflowed usize"))?;
            let Some(value) = sequence.next_element_seed(self.prospective_child(JsonContainerKind::Sequence, next))?
            else {
                break;
            };
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    /// Accounts and materializes a JSON object from map events.
    fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        self.transaction
            .try_enter_container(JsonContainerKind::Map, self.depth)
            .map_err(A::Error::custom)?;
        let mut values = Map::new();
        let mut entries = 0_usize;
        loop {
            let next = entries
                .checked_add(1)
                .ok_or_else(|| A::Error::custom("JSON map entry count overflowed usize"))?;
            let Some(key) = map.next_key_seed(JsonKeySeed {
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
        Ok(Value::Object(values))
    }
}
