// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Compound state for single-pass budget-aware JSON encoding.
// qubit-style: allow source-test-pair
// qubit-style: allow multiple-public-types
// qubit-style: allow explicit-imports

use std::cell::RefCell;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use serde::Serialize;
use serde::Serializer;
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
use super::json_encode_context::JsonEncodeContext;
use super::json_encode_serializer::JsonEncodeSerializer;

/// Special serde_json struct encoding recognized by the wrapper.
#[derive(Clone, Copy)]
enum PrivateStruct {
    /// Regular JSON object encoding.
    Regular,

    /// Arbitrary-precision JSON number encoding.
    Number,

    /// Raw JSON fragment encoding.
    RawValue,
}

/// Wraps one nested value so the underlying compound serializer re-enters the
/// budget-aware serializer before traversing it.
pub(super) struct BudgetedValue<'a, 'budget, 'context, T, R, Q>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Original nested value.
    value: &'a T,

    /// Shared mutable budget state for the serialization traversal.
    context: &'context RefCell<JsonEncodeContext<'budget, R, Q>>,

    /// Root-inclusive depth assigned to the nested value.
    depth: usize,
}

impl<'a, 'budget, 'context, T, R, Q> BudgetedValue<'a, 'budget, 'context, T, R, Q>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Creates a nested value wrapper bound to a shared budget context.
    pub(super) const fn new(
        value: &'a T,
        context: &'context RefCell<JsonEncodeContext<'budget, R, Q>>,
        depth: usize,
    ) -> Self {
        Self {
            value,
            context,
            depth,
        }
    }
}

impl<T, R, Q> Serialize for BudgetedValue<'_, '_, '_, T, R, Q>
where
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Serializes the wrapped value through a child budget decorator.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value.serialize(JsonEncodeSerializer::with_context(
            serializer,
            self.context,
            self.depth,
        ))
    }
}

/// Wraps a Serde compound serializer and checks container operations before
/// delegating them.
pub(in crate::budget) struct JsonEncodeCompound<'a, 'context, C, R, Q>
where
    Q: ResourceQuantity,
{
    /// Underlying Serde compound serializer.
    inner: C,

    /// Shared mutable budget state for the serialization traversal.
    context: &'context RefCell<JsonEncodeContext<'a, R, Q>>,

    /// Root-inclusive depth assigned to nested values.
    child_depth: usize,

    /// Number of sequence items or map entries observed so far.
    observed: usize,

    /// Private serde_json encoding represented by this compound.
    private: PrivateStruct,
}

impl<'a, 'context, C, R, Q> JsonEncodeCompound<'a, 'context, C, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a wrapper for a regular JSON array or object compound.
    pub(super) const fn new(
        inner: C,
        context: &'context RefCell<JsonEncodeContext<'a, R, Q>>,
        child_depth: usize,
    ) -> Self {
        Self {
            inner,
            context,
            child_depth,
            observed: 0,
            private: PrivateStruct::Regular,
        }
    }

    /// Creates a wrapper for serde_json's private number compound.
    pub(super) const fn number(
        inner: C,
        context: &'context RefCell<JsonEncodeContext<'a, R, Q>>,
        depth: usize,
    ) -> Self {
        Self {
            inner,
            context,
            child_depth: depth,
            observed: 0,
            private: PrivateStruct::Number,
        }
    }

    /// Creates a wrapper for serde_json's private raw-value compound.
    pub(super) const fn raw_value(
        inner: C,
        context: &'context RefCell<JsonEncodeContext<'a, R, Q>>,
        depth: usize,
    ) -> Self {
        Self {
            inner,
            context,
            child_depth: depth,
            observed: 0,
            private: PrivateStruct::RawValue,
        }
    }

    /// Records the original budget error and maps it into the compound error.
    fn record<E>(&mut self, result: Result<(), MeasuredBudgetError<R, Q>>) -> Result<(), E>
    where
        E: Error,
    {
        self.context.borrow_mut().record(result)
    }

    /// Checks the next observed sequence element.
    fn next_sequence<E>(&mut self) -> Result<(), E>
    where
        E: Error,
    {
        self.observed = self.observed.saturating_add(1);
        let result = self
            .context
            .borrow()
            .budget
            .check_sequence_items_usize(self.observed);
        self.record(result)
    }

    /// Checks the next observed map or struct entry.
    fn next_map_entry<E>(&mut self) -> Result<(), E>
    where
        E: Error,
    {
        self.observed = self.observed.saturating_add(1);
        let result = self
            .context
            .borrow()
            .budget
            .check_map_entries_usize(self.observed);
        self.record(result)
    }

    /// Confirms the final observed sequence length before completion.
    fn finish_sequence<E>(&mut self) -> Result<(), E>
    where
        E: Error,
    {
        let result = self
            .context
            .borrow()
            .budget
            .check_sequence_items_usize(self.observed);
        self.record(result)
    }

    /// Confirms the final observed map length before completion.
    fn finish_map<E>(&mut self) -> Result<(), E>
    where
        E: Error,
    {
        let result = self
            .context
            .borrow()
            .budget
            .check_map_entries_usize(self.observed);
        self.record(result)
    }
}

impl<C, R, Q> SerializeSeq for JsonEncodeCompound<'_, '_, C, R, Q>
where
    C: SerializeSeq,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Checks the observed count, then serializes one decorated child value.
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.next_sequence()?;
        let value = BudgetedValue::new(value, self.context, self.child_depth);
        self.inner.serialize_element(&value)
    }

    /// Completes the underlying sequence.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish_sequence()?;
        self.inner.end()
    }
}

impl<C, R, Q> SerializeTuple for JsonEncodeCompound<'_, '_, C, R, Q>
where
    C: SerializeTuple,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = C::Ok;
    type Error = C::Error;

    /// Serializes one decorated tuple element.
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.next_sequence()?;
        let value = BudgetedValue::new(value, self.context, self.child_depth);
        self.inner.serialize_element(&value)
    }

    /// Completes the underlying tuple.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish_sequence()?;
        self.inner.end()
    }
}

impl<C, R, Q> SerializeTupleStruct for JsonEncodeCompound<'_, '_, C, R, Q>
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
        let value = BudgetedValue::new(value, self.context, self.child_depth);
        self.inner.serialize_field(&value)
    }

    /// Completes the underlying tuple struct.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish_sequence()?;
        self.inner.end()
    }
}

impl<C, R, Q> SerializeTupleVariant for JsonEncodeCompound<'_, '_, C, R, Q>
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
        let value = BudgetedValue::new(value, self.context, self.child_depth);
        self.inner.serialize_field(&value)
    }

    /// Completes the underlying tuple variant.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish_sequence()?;
        self.inner.end()
    }
}

impl<C, R, Q> SerializeMap for JsonEncodeCompound<'_, '_, C, R, Q>
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
        self.next_map_entry()?;
        let key = BudgetedKey::new(key, self.context);
        self.inner.serialize_key(&key)
    }

    /// Serializes one map value through a child budget decorator.
    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let value = BudgetedValue::new(value, self.context, self.child_depth);
        self.inner.serialize_value(&value)
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

impl<C, R, Q> SerializeStruct for JsonEncodeCompound<'_, '_, C, R, Q>
where
    C: SerializeStruct,
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
        match self.private {
            PrivateStruct::Number => {
                let value = BudgetedPrivateValue::number(value, self.context);
                return self.inner.serialize_field(key, &value);
            }
            PrivateStruct::RawValue => {
                let value = BudgetedPrivateValue::raw_value(value, self.context, self.child_depth);
                return self.inner.serialize_field(key, &value);
            }
            PrivateStruct::Regular => self.next_map_entry()?,
        }
        let key_result = self
            .context
            .borrow_mut()
            .budget
            .consume_key_bytes_usize(key.len());
        self.record(key_result)?;
        let value = BudgetedValue::new(value, self.context, self.child_depth);
        self.inner.serialize_field(key, &value)
    }

    /// Skips one field exactly as the underlying serializer requests.
    #[inline(always)]
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        self.inner.skip_field(key)
    }

    /// Completes the underlying struct.
    #[inline(always)]
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        if matches!(self.private, PrivateStruct::Regular) {
            self.finish_map()?;
        }
        self.inner.end()
    }
}

impl<C, R, Q> SerializeStructVariant for JsonEncodeCompound<'_, '_, C, R, Q>
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
        let key_result = self
            .context
            .borrow_mut()
            .budget
            .consume_key_bytes_usize(key.len());
        self.record(key_result)?;
        let value = BudgetedValue::new(value, self.context, self.child_depth);
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
