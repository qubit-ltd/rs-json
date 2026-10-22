// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Compound state for single-pass budget-aware JSON encoding.

use std::cell::RefCell;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonContainerKind;
use qubit_budget::json::JsonMeasurement;
use serde::Serialize;
use serde::ser::Error;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;
use serde::ser::SerializeStructVariant;
use serde::ser::SerializeTuple;
use serde::ser::SerializeTupleStruct;
use serde::ser::SerializeTupleVariant;

use super::budgeted_key::BudgetedKey;
use super::budgeted_private_value::BudgetedPrivateValue;
use super::budgeted_value::BudgetedValue;
use super::json_encode_context::JsonEncodeContext;
use crate::encode::JsonSerializationErrorKind;
use crate::encode::JsonSerializerStateError;
use crate::internal::JsonPrivateProtocolKind;
use crate::internal::JsonPrivateProtocolState;
use crate::internal::SERDE_JSON_RAW_VALUE_TOKEN;

/// Wraps a Serde compound serializer and checks container operations before
/// delegating them.
pub(in crate::encode) struct JsonEncodeCompound<
    'transaction,
    'budget,
    'context,
    C,
    R,
    Q,
    const VALUE_LIMITS: bool,
> where
    Q: ResourceQuantity,
{
    /// Underlying Serde compound serializer.
    inner: C,

    /// Shared mutable budget state for the serialization traversal.
    context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,

    /// Root-inclusive depth assigned to nested values.
    child_depth: usize,

    /// Number of sequence items or map entries observed so far.
    observed: usize,

    /// Whether a map key has been accepted without its corresponding value.
    map_key_pending: bool,

    /// Private serde_json encoding represented by this compound.
    private_kind: Option<JsonPrivateProtocolKind>,

    /// Validated private protocol lifecycle, when this is a private struct.
    private_state: Option<JsonPrivateProtocolState>,
}

impl<'transaction, 'budget, 'context, C, R, Q, const VALUE_LIMITS: bool>
    JsonEncodeCompound<'transaction, 'budget, 'context, C, R, Q, VALUE_LIMITS>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a wrapper for a regular JSON array or object compound.
    #[inline]
    pub(super) const fn new(
        inner: C,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
        child_depth: usize,
    ) -> Self {
        Self {
            inner,
            context,
            child_depth,
            observed: 0,
            map_key_pending: false,
            private_kind: None,
            private_state: None,
        }
    }

    /// Creates a wrapper for serde_json's private raw-value compound.
    pub(super) const fn private(
        inner: C,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
        depth: usize,
        kind: JsonPrivateProtocolKind,
        state: JsonPrivateProtocolState,
    ) -> Self {
        Self {
            inner,
            context,
            child_depth: depth,
            observed: 0,
            map_key_pending: false,
            private_kind: Some(kind),
            private_state: Some(state),
        }
    }

    /// Checks the next observed sequence element.
    #[inline(always)]
    fn next_sequence<E>(&mut self) -> Result<(), E>
    where
        E: Error,
    {
        if !VALUE_LIMITS {
            return Ok(());
        }
        let next = self
            .observed
            .checked_add(1)
            .ok_or_else(|| E::custom("JSON sequence item count overflowed usize"))?;
        self.context
            .borrow_mut()
            .check_container_count(JsonContainerKind::Sequence, next)?;
        self.observed = next;
        Ok(())
    }

    /// Checks the next observed map or struct entry.
    #[inline(always)]
    fn next_map_entry<E>(&mut self) -> Result<(), E>
    where
        E: Error,
    {
        if !VALUE_LIMITS {
            return Ok(());
        }
        let next = self
            .observed
            .checked_add(1)
            .ok_or_else(|| E::custom("JSON map entry count overflowed usize"))?;
        self.context
            .borrow_mut()
            .check_container_count(JsonContainerKind::Map, next)?;
        self.observed = next;
        Ok(())
    }

    /// Confirms the final observed sequence length before completion.
    fn finish_sequence<E>(&mut self) -> Result<(), E>
    where
        E: Error,
    {
        Ok(())
    }

    /// Confirms the final observed map length before completion.
    fn finish_map<E>(&mut self) -> Result<(), E>
    where
        E: Error,
    {
        if self.map_key_pending {
            return Err(self.serialization_error(JsonSerializerStateError::MapEndedWithPendingKey));
        }
        Ok(())
    }

    /// Records and returns one invalid map serializer state.
    fn serialization_error<E>(&self, reason: JsonSerializerStateError) -> E
    where
        E: Error,
    {
        self.context
            .borrow_mut()
            .serialization_error(JsonSerializationErrorKind::InvalidSerializerState { reason })
    }

    /// Records the current private protocol's stable failure.
    fn private_protocol_error<E>(&self) -> E
    where
        E: Error,
    {
        let kind = self.private_kind.expect("private protocol kind must exist");
        self.context.borrow_mut().serialization_error(
            JsonSerializationErrorKind::InvalidSerializerState {
                reason: kind.invalid_state(),
            },
        )
    }
}

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeSeq
    for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeSeq,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Checks the observed count, then serializes one decorated child value.
    #[inline(always)]
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.next_sequence()?;
        let value =
            BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_element(&value)
    }

    /// Completes the underlying sequence.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish_sequence()?;
        self.inner.end()
    }
}

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeTuple
    for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeTuple,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Serializes one decorated tuple element.
    #[inline(always)]
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.next_sequence()?;
        let value =
            BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_element(&value)
    }

    /// Completes the underlying tuple.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish_sequence()?;
        self.inner.end()
    }
}

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeTupleStruct
    for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeTupleStruct,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Serializes one decorated tuple-struct field.
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.next_sequence()?;
        let value =
            BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_field(&value)
    }

    /// Completes the underlying tuple struct.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish_sequence()?;
        self.inner.end()
    }
}

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeTupleVariant
    for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeTupleVariant,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Serializes one decorated tuple-variant field.
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.next_sequence()?;
        let value =
            BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_field(&value)
    }

    /// Completes the underlying tuple variant.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish_sequence()?;
        self.inner.end()
    }
}

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeMap
    for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeMap,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Checks the entry count and key before delegating key serialization.
    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        if self.map_key_pending {
            return Err(self.serialization_error(JsonSerializerStateError::MapKeyAlreadyPending));
        }
        self.next_map_entry()?;
        let key = BudgetedKey::<_, _, _, VALUE_LIMITS>::new(key, self.context);
        self.inner.serialize_key(&key)?;
        self.map_key_pending = true;
        Ok(())
    }

    /// Serializes one map value through a child budget decorator.
    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        if !self.map_key_pending {
            return Err(self.serialization_error(JsonSerializerStateError::MapValueWithoutKey));
        }
        let value =
            BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_value(&value)?;
        self.map_key_pending = false;
        Ok(())
    }

    /// Checks and serializes one complete map entry.
    fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<(), Self::Error>
    where
        K: Serialize + ?Sized,
        V: Serialize + ?Sized,
    {
        self.serialize_key(key)?;
        self.serialize_value(value)
    }

    /// Completes the underlying map.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish_map()?;
        self.inner.end()
    }
}

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeStruct
    for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeStruct,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Checks one field key and serializes its decorated value.
    #[inline(always)]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        if let Some(kind) = self.private_kind {
            let state = self
                .private_state
                .as_mut()
                .expect("private state must exist");
            if state.begin(key).is_err() {
                return Err(self.private_protocol_error());
            }
            let payload = match kind {
                JsonPrivateProtocolKind::RawValue => {
                    BudgetedPrivateValue::raw_value(value, self.context, self.child_depth)
                }
                JsonPrivateProtocolKind::Number => {
                    BudgetedPrivateValue::number(value, self.context, self.child_depth)
                }
            };
            let inner_key = if kind == JsonPrivateProtocolKind::Number {
                SERDE_JSON_RAW_VALUE_TOKEN
            } else {
                key
            };
            if let Err(error) = self.inner.serialize_field(inner_key, &payload) {
                self.private_state
                    .as_mut()
                    .expect("private state must exist")
                    .fail();
                let _: Self::Error = self.private_protocol_error();
                return Err(error);
            }
            if self
                .private_state
                .as_mut()
                .expect("private state must exist")
                .complete()
                .is_err()
            {
                return Err(self.private_protocol_error());
            }
            return Ok(());
        }
        self.next_map_entry()?;
        if VALUE_LIMITS {
            self.context
                .borrow_mut()
                .admit(JsonMeasurement::Key { bytes: key.len() })?;
        }
        let value =
            BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_field(key, &value)
    }

    /// Skips one field exactly as the underlying serializer requests.
    #[inline(always)]
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        if self.private_state.is_some() {
            self.private_state
                .as_mut()
                .expect("private state must exist")
                .fail();
            return Err(self.private_protocol_error());
        }
        self.inner.skip_field(key)
    }

    /// Completes the underlying struct.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        if let Some(state) = self.private_state.take() {
            if state.finish().is_err() {
                return Err(self.private_protocol_error());
            }
        } else {
            self.finish_map()?;
        }
        self.inner.end()
    }
}

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeStructVariant
    for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeStructVariant,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Checks one field key and serializes its decorated value.
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.next_map_entry()?;
        if VALUE_LIMITS {
            self.context
                .borrow_mut()
                .admit(JsonMeasurement::Key { bytes: key.len() })?;
        }
        let value =
            BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_field(key, &value)
    }

    /// Skips one variant field exactly as the underlying serializer requests.
    #[inline(always)]
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        self.inner.skip_field(key)
    }

    /// Completes the underlying struct variant.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish_map()?;
        self.inner.end()
    }
}
