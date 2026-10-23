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

use super::BudgetedKey;
use super::BudgetedPrivateValue;
use super::BudgetedValue;
use super::JsonEncodeContext;
use crate::encode::JsonSerializationErrorKind;
use crate::encode::JsonSerializerStateError;
use crate::internal::JsonPrivateProtocolKind;
use crate::internal::JsonPrivateProtocolState;
use crate::internal::SERDE_JSON_RAW_VALUE_TOKEN;

/// Wraps a Serde compound serializer and checks container operations before
/// delegating them.
///
/// # Type Parameters
///
/// * `C` - Underlying Serde compound serializer.
/// * `R` - Resource identifier used by the shared encode budget.
/// * `Q` - Quantity representation used by the shared encode budget.
/// * `VALUE_LIMITS` - Whether value-level limits are enforced.
pub(in crate::encode) struct JsonEncodeCompound<'transaction, 'budget, 'context, C, R, Q, const VALUE_LIMITS: bool>
where
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

    /// Whether the serializer declared this compound empty.
    declared_empty: bool,

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
    ///
    /// # Parameters
    ///
    /// * `inner` - Serde compound serializer that receives admitted values.
    /// * `context` - Shared mutable state for this serialization traversal.
    /// * `child_depth` - Root-inclusive depth assigned to child values.
    /// * `declared_empty` - Whether the serializer declared zero items or
    ///   fields for this compound.
    ///
    /// # Returns
    ///
    /// A compound wrapper with regular JSON protocol state initialized.
    #[must_use]
    #[inline]
    pub(super) const fn new(
        inner: C,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
        child_depth: usize,
        declared_empty: bool,
    ) -> Self {
        Self {
            inner,
            context,
            child_depth,
            observed: 0,
            declared_empty,
            map_key_pending: false,
            private_kind: None,
            private_state: None,
        }
    }

    /// Creates a wrapper for serde_json's private raw-value compound.
    ///
    /// # Parameters
    ///
    /// * `inner` - Serde struct serializer for the private compound.
    /// * `context` - Shared mutable state for this serialization traversal.
    /// * `depth` - Root-inclusive depth assigned to the private payload.
    /// * `kind` - Private serde_json protocol represented by the compound.
    /// * `state` - Validated lifecycle state for the private protocol.
    ///
    /// # Returns
    ///
    /// A compound wrapper initialized for the selected private protocol.
    #[must_use]
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
            declared_empty: false,
            map_key_pending: false,
            private_kind: Some(kind),
            private_state: Some(state),
        }
    }

    /// Checks the next observed sequence element.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serde error type used to report count failures.
    ///
    /// # Returns
    ///
    /// `Ok(())` when limits are disabled or the next element is admitted.
    ///
    /// # Errors
    ///
    /// Returns `E` when the count overflows or the sequence limit is exceeded.
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
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serde error type used to report count failures.
    ///
    /// # Returns
    ///
    /// `Ok(())` when limits are disabled or the next entry is admitted.
    ///
    /// # Errors
    ///
    /// Returns `E` when the count overflows or the map limit is exceeded.
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

    /// Rejects completion when a map key has no matching value.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serde error type used to report an incomplete map entry.
    ///
    /// # Returns
    ///
    /// `Ok(())` when no key is pending.
    ///
    /// # Errors
    ///
    /// Returns `E` when a key was serialized without its corresponding value.
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
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serde error type constructed from the recorded state error.
    ///
    /// # Parameters
    ///
    /// * `reason` - Invalid map state to preserve in the serialization error.
    ///
    /// # Returns
    ///
    /// An `E` containing the recorded invalid-state reason.
    fn serialization_error<E>(&self, reason: JsonSerializerStateError) -> E
    where
        E: Error,
    {
        self.context
            .borrow_mut()
            .serialization_error(JsonSerializationErrorKind::InvalidSerializerState { reason })
    }

    /// Rejects a callback for a compound declared with zero items or fields.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the compound declaration permits callbacks.
    ///
    /// # Errors
    ///
    /// Returns a Serde error containing
    /// [`JsonSerializerStateError::UnexpectedCompound`] when the compound
    /// declared no items or fields.
    #[inline(always)]
    fn reject_empty_callback<E>(&self) -> Result<(), E>
    where
        E: Error,
    {
        if self.declared_empty {
            return Err(self.serialization_error(JsonSerializerStateError::UnexpectedCompound));
        }
        Ok(())
    }

    /// Records the current private protocol's stable failure.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serde error type constructed from the protocol error.
    ///
    /// # Returns
    ///
    /// An `E` containing the private protocol's stable invalid-state reason.
    ///
    /// # Panics
    ///
    /// Panics if called for a regular compound without a private protocol kind.
    fn private_protocol_error<E>(&self) -> E
    where
        E: Error,
    {
        let kind = self.private_kind.expect("private protocol kind must exist");
        self.context
            .borrow_mut()
            .serialization_error(JsonSerializationErrorKind::InvalidSerializerState {
                reason: kind.invalid_state(),
            })
    }
}

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeSeq for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeSeq,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Checks the observed count, then serializes one decorated child value.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Value serialized as the next sequence element.
    ///
    /// # Parameters
    ///
    /// * `value` - Element whose serialization is budgeted before delegation.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the underlying serializer accepts the element.
    ///
    /// # Errors
    ///
    /// Returns the serializer error for an exceeded count or rejected element.
    #[inline(always)]
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.reject_empty_callback()?;
        self.next_sequence()?;
        let value = BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_element(&value)
    }

    /// Completes the underlying sequence.
    ///
    /// # Returns
    ///
    /// The underlying serializer's completed sequence result.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying serializer cannot finish the
    /// sequence.
    #[inline(always)]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeTuple for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeTuple,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Serializes one decorated tuple element.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Value serialized as the next tuple element.
    ///
    /// # Parameters
    ///
    /// * `value` - Element whose serialization is budgeted before delegation.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the underlying serializer accepts the element.
    ///
    /// # Errors
    ///
    /// Returns the serializer error for an exceeded count or rejected element.
    #[inline(always)]
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.reject_empty_callback()?;
        self.next_sequence()?;
        let value = BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_element(&value)
    }

    /// Completes the underlying tuple.
    ///
    /// # Returns
    ///
    /// The underlying serializer's completed tuple result.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying serializer cannot finish the tuple.
    #[inline(always)]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeTupleStruct for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeTupleStruct,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Serializes one decorated tuple-struct field.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Value serialized as the next tuple-struct field.
    ///
    /// # Parameters
    ///
    /// * `value` - Field whose serialization is budgeted before delegation.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the underlying serializer accepts the field.
    ///
    /// # Errors
    ///
    /// Returns the serializer error for an exceeded count or rejected field.
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.reject_empty_callback()?;
        self.next_sequence()?;
        let value = BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_field(&value)
    }

    /// Completes the underlying tuple struct.
    ///
    /// # Returns
    ///
    /// The underlying serializer's completed tuple-struct result.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying serializer cannot finish the tuple
    /// struct.
    #[inline(always)]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeTupleVariant for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeTupleVariant,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Serializes one decorated tuple-variant field.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Value serialized as the next tuple-variant field.
    ///
    /// # Parameters
    ///
    /// * `value` - Field whose serialization is budgeted before delegation.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the underlying serializer accepts the field.
    ///
    /// # Errors
    ///
    /// Returns the serializer error for an exceeded count or rejected field.
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.reject_empty_callback()?;
        self.next_sequence()?;
        let value = BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_field(&value)
    }

    /// Completes the underlying tuple variant.
    ///
    /// # Returns
    ///
    /// The underlying serializer's completed tuple-variant result.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying serializer cannot finish the tuple
    /// variant.
    #[inline(always)]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.inner.end()
    }
}

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeMap for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeMap,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Checks the entry count and key before delegating key serialization.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Value serialized as the map key.
    ///
    /// # Parameters
    ///
    /// * `key` - Key whose count and serialization are checked before
    ///   delegation.
    ///
    /// # Returns
    ///
    /// `Ok(())` and marks the map as awaiting a value after key acceptance.
    ///
    /// # Errors
    ///
    /// Returns an error when a key is already pending, the count is rejected,
    /// or the underlying serializer rejects the key.
    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.reject_empty_callback()?;
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
    ///
    /// # Type Parameters
    ///
    /// * `T` - Value serialized for the pending map key.
    ///
    /// # Parameters
    ///
    /// * `value` - Value whose serialization is budgeted before delegation.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the underlying serializer accepts the value and clears
    /// the pending-key state.
    ///
    /// # Errors
    ///
    /// Returns an error when no key is pending or serialization fails.
    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.reject_empty_callback()?;
        if !self.map_key_pending {
            return Err(self.serialization_error(JsonSerializerStateError::MapValueWithoutKey));
        }
        let value = BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_value(&value)?;
        self.map_key_pending = false;
        Ok(())
    }

    /// Checks and serializes one complete map entry.
    ///
    /// # Type Parameters
    ///
    /// * `K` - Value serialized as the key.
    /// * `V` - Value serialized as the entry value.
    ///
    /// # Parameters
    ///
    /// * `key` - Key to serialize before the entry value.
    /// * `value` - Value serialized after the key is accepted.
    ///
    /// # Returns
    ///
    /// `Ok(())` after both parts are accepted by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an error when the count, key, or value is rejected.
    fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<(), Self::Error>
    where
        K: Serialize + ?Sized,
        V: Serialize + ?Sized,
    {
        self.reject_empty_callback()?;
        self.serialize_key(key)?;
        self.serialize_value(value)
    }

    /// Completes the underlying map.
    ///
    /// # Returns
    ///
    /// The underlying serializer's completed map result.
    ///
    /// # Errors
    ///
    /// Returns an error if a key has no matching value or the serializer fails.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish_map()?;
        self.inner.end()
    }
}

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeStruct for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeStruct,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Checks one field key and serializes its decorated value.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Value serialized for this struct field.
    ///
    /// # Parameters
    ///
    /// * `key` - Static field name admitted against the key budget.
    /// * `value` - Field value serialized through its child budget decorator.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the field is accepted by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid private protocol, exceeded field count,
    /// rejected key/value, or underlying serializer failure.
    #[inline(always)]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.reject_empty_callback()?;
        if let Some(kind) = self.private_kind {
            let state = self.private_state.as_mut().expect("private state must exist");
            if state.begin(key).is_err() {
                return Err(self.private_protocol_error());
            }
            let payload = match kind {
                JsonPrivateProtocolKind::RawValue => {
                    BudgetedPrivateValue::raw_value(value, self.context, self.child_depth)
                }
                JsonPrivateProtocolKind::Number => BudgetedPrivateValue::number(value, self.context, self.child_depth),
            };
            let inner_key = if kind == JsonPrivateProtocolKind::Number {
                SERDE_JSON_RAW_VALUE_TOKEN
            } else {
                key
            };
            if let Err(error) = self.inner.serialize_field(inner_key, &payload) {
                self.private_state.as_mut().expect("private state must exist").fail();
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
        let value = BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_field(key, &value)
    }

    /// Skips one field exactly as the underlying serializer requests.
    ///
    /// # Parameters
    ///
    /// * `key` - Static field name the underlying serializer elects to skip.
    ///
    /// # Returns
    ///
    /// The underlying serializer's skip result.
    ///
    /// # Errors
    ///
    /// Returns an error if a private protocol field is skipped or the
    /// underlying serializer rejects the operation.
    #[inline(always)]
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        if let Some(state) = self.private_state.as_mut() {
            state.fail();
            return Err(self.private_protocol_error());
        }
        self.inner.skip_field(key)
    }

    /// Completes the underlying struct.
    ///
    /// # Returns
    ///
    /// The underlying serializer's completed struct result.
    ///
    /// # Errors
    ///
    /// Returns an error for an incomplete private protocol, pending map key,
    /// or underlying serializer failure.
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

impl<C, R, Q, const VALUE_LIMITS: bool> SerializeStructVariant for JsonEncodeCompound<'_, '_, '_, C, R, Q, VALUE_LIMITS>
where
    C: SerializeStructVariant,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Checks one field key and serializes its decorated value.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Value serialized for this struct-variant field.
    ///
    /// # Parameters
    ///
    /// * `key` - Static field name admitted against the key budget.
    /// * `value` - Field value serialized through its child budget decorator.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the field is accepted by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an error for an exceeded field count, rejected key/value, or
    /// underlying serializer failure.
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.reject_empty_callback()?;
        self.next_map_entry()?;
        if VALUE_LIMITS {
            self.context
                .borrow_mut()
                .admit(JsonMeasurement::Key { bytes: key.len() })?;
        }
        let value = BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.child_depth);
        self.inner.serialize_field(key, &value)
    }

    /// Skips one variant field exactly as the underlying serializer requests.
    ///
    /// # Parameters
    ///
    /// * `key` - Static field name the underlying serializer elects to skip.
    ///
    /// # Returns
    ///
    /// The underlying serializer's skip result.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying serializer rejects the operation.
    #[inline(always)]
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        self.inner.skip_field(key)
    }

    /// Completes the underlying struct variant.
    ///
    /// # Returns
    ///
    /// The underlying serializer's completed struct-variant result.
    ///
    /// # Errors
    ///
    /// Returns an error for a pending map key or underlying serializer failure.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish_map()?;
        self.inner.end()
    }
}
