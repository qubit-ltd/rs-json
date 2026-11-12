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
use crate::internal::StrictJsonNumber;

/// Visitor used to construct and account one JSON value.
///
/// # Type Parameters
///
/// * `R` - Resource identity retained by the transaction's limits and errors.
/// * `Q` - Exact unsigned quantity used for measurements and accounting.
pub(in crate::value) struct JsonValueVisitor<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Transaction receiving measurements for the value under construction.
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,

    /// Root-inclusive depth of the value currently being visited.
    depth: usize,

    /// Whether duplicate decoded object keys are rejected before their values.
    reject_duplicate_keys: bool,
}

impl<'transaction, 'budget, R, Q> JsonValueVisitor<'transaction, 'budget, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// Creates a visitor that accounts a value at `depth`.
    ///
    /// # Parameters
    ///
    /// * `transaction` - Transaction that receives measurements for the value.
    /// * `depth` - Root-inclusive depth assigned to the value.
    /// * `reject_duplicate_keys` - Whether repeated decoded object keys fail.
    #[inline]
    pub(in crate::value) fn new(
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
        depth: usize,
        reject_duplicate_keys: bool,
    ) -> Self {
        Self {
            transaction,
            depth,
            reject_duplicate_keys,
        }
    }

    /// Stages one complete JSON measurement.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used to report accounting failures.
    ///
    /// # Parameters
    ///
    /// * `measurement` - Measurement to admit into the transaction.
    fn admit<E>(&mut self, measurement: JsonMeasurement) -> Result<(), E>
    where
        E: Error,
    {
        self.transaction.try_admit(measurement).map_err(E::custom)
    }

    /// Charges a number's decoded text representation.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used to report accounting failures.
    ///
    /// # Parameters
    ///
    /// * `number` - JSON number whose representation is charged.
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
        JsonValueVisitor::new(
            self.transaction,
            self.depth.saturating_add(1),
            self.reject_duplicate_keys,
        )
    }

    /// Creates a child seed that checks a container limit before decoding.
    ///
    /// # Parameters
    ///
    /// * `kind` - Container kind whose prospective child is checked.
    /// * `prospective` - One-based child count to validate before decoding.
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
            reject_duplicate_keys: self.reject_duplicate_keys,
        }
    }
}

impl<'de, R, Q> DeserializeSeed<'de> for JsonValueVisitor<'_, '_, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// JSON value produced after the seed delegates deserialization.
    type Value = Value;

    /// Starts an `deserialize_any` visit for the requested JSON value.
    ///
    /// # Type Parameters
    ///
    /// * `D` - Deserializer supplying the JSON value.
    ///
    /// # Parameters
    ///
    /// * `deserializer` - Source that presents the value to this seed.
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
    /// JSON value assembled from the deserializer's visitor events.
    type Value = Value;

    /// Describes the JSON value shape accepted by this visitor.
    ///
    /// # Parameters
    ///
    /// * `formatter` - Formatter receiving the human-readable expectation.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a value representable as JSON")
    }

    /// Accounts and constructs a JSON boolean.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used for accounting failures.
    ///
    /// # Parameters
    ///
    /// * `value` - Boolean supplied by the deserializer.
    fn visit_bool<E>(mut self, value: bool) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Boolean { depth: self.depth })?;
        Ok(Value::Bool(value))
    }

    /// Accounts and constructs a signed JSON integer.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used for accounting failures.
    ///
    /// # Parameters
    ///
    /// * `value` - Signed integer supplied by the deserializer.
    fn visit_i64<E>(mut self, value: i64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.enter_number(Number::from(value))
    }

    /// Rejects unsupported wide signed integers or accounts the representable
    /// value.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used for conversion failures.
    ///
    /// # Parameters
    ///
    /// * `value` - Wide signed integer supplied by the deserializer.
    #[inline(never)]
    fn visit_i128<E>(mut self, value: i128) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let number = StrictJsonNumber::from_i128(value).map_err(E::custom)?;
        self.enter_number(number)
    }

    /// Accounts and constructs an unsigned JSON integer.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used for accounting failures.
    ///
    /// # Parameters
    ///
    /// * `value` - Unsigned integer supplied by the deserializer.
    fn visit_u64<E>(mut self, value: u64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.enter_number(Number::from(value))
    }

    /// Rejects unsupported wide unsigned integers or accounts the representable
    /// value.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used for conversion failures.
    ///
    /// # Parameters
    ///
    /// * `value` - Wide unsigned integer supplied by the deserializer.
    #[inline(never)]
    fn visit_u128<E>(mut self, value: u128) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let number = StrictJsonNumber::from_u128(value).map_err(E::custom)?;
        self.enter_number(number)
    }

    /// Rejects non-finite floats or accounts the representable JSON number.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used for conversion failures.
    ///
    /// # Parameters
    ///
    /// * `value` - Floating-point value supplied by the deserializer.
    fn visit_f64<E>(mut self, value: f64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let number =
            Number::from_f64(value).ok_or_else(|| E::custom("non-finite float is not representable as JSON"))?;
        self.enter_number(number)
    }

    /// Accounts and copies a borrowed JSON string.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used for accounting failures.
    ///
    /// # Parameters
    ///
    /// * `value` - Borrowed string supplied by the deserializer.
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
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used for accounting failures.
    ///
    /// # Parameters
    ///
    /// * `value` - Owned string supplied by the deserializer.
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
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used for accounting failures.
    fn visit_none<E>(mut self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
        Ok(Value::Null)
    }

    /// Accounts a unit value as JSON null.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used for accounting failures.
    fn visit_unit<E>(mut self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
        Ok(Value::Null)
    }

    /// Delegates an optional value to the same JSON visitor.
    ///
    /// # Type Parameters
    ///
    /// * `D` - Deserializer supplying the optional payload.
    ///
    /// # Parameters
    ///
    /// * `deserializer` - Source that presents the optional payload.
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    /// Delegates a newtype payload to the same JSON visitor.
    ///
    /// # Type Parameters
    ///
    /// * `D` - Deserializer supplying the newtype payload.
    ///
    /// # Parameters
    ///
    /// * `deserializer` - Source that presents the wrapped value.
    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    /// Accounts and materializes a JSON array from sequence events.
    ///
    /// # Type Parameters
    ///
    /// * `A` - Sequence access implementation supplied by the deserializer.
    ///
    /// # Parameters
    ///
    /// * `sequence` - Accessor yielding the array's elements.
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
    ///
    /// # Type Parameters
    ///
    /// * `A` - Map access implementation supplied by the deserializer.
    ///
    /// # Parameters
    ///
    /// * `map` - Accessor yielding the object's keys and values.
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
            if self.reject_duplicate_keys && values.contains_key(&key) {
                return Err(A::Error::custom("duplicate JSON object key"));
            }
            let value = map.next_value_seed(self.child())?;
            entries = next;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
