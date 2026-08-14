// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serde serializer decorator with online JSON budget checks.
// qubit-style: allow source-test-pair
// qubit-style: allow multiple-public-types
// qubit-style: allow explicit-imports

use std::cell::RefCell;
use std::fmt::Display;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error;

use super::display_budget_kind::DisplayBudgetKind;
use super::json_encode_compound::BudgetedValue;
use super::json_encode_compound::JsonEncodeCompound;
use super::json_encode_context::JsonEncodeContext;
use super::json_encode_context::collect_display;
use super::private_struct_kind::PrivateStructKind;
use super::serde_json_compat::classify_private_struct;

/// Decorates one Serde serializer with eager JSON budget checks.
pub(in crate::budget) struct JsonEncodeSerializer<'a, 'context, S, R, Q>
where
    Q: ResourceQuantity,
{
    /// Underlying serializer that emits JSON events.
    inner: S,

    /// Shared mutable state for this traversal.
    context: &'context RefCell<JsonEncodeContext<'a, R, Q>>,

    /// Root-inclusive depth assigned to the current value.
    depth: usize,
}

impl<'a, 'context, S, R, Q> JsonEncodeSerializer<'a, 'context, S, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a root decorator at depth one.
    ///
    /// # Parameters
    ///
    /// * `inner` - Underlying serializer that emits the JSON document.
    /// * `context` - Shared state for value and output accounting.
    ///
    /// # Returns
    ///
    /// A decorator positioned at the root JSON value.
    pub(in crate::budget) fn new(
        inner: S,
        context: &'context RefCell<JsonEncodeContext<'a, R, Q>>,
    ) -> Self {
        Self {
            inner,
            context,
            depth: 1,
        }
    }

    /// Creates a nested decorator sharing an existing traversal context.
    pub(super) const fn with_context(
        inner: S,
        context: &'context RefCell<JsonEncodeContext<'a, R, Q>>,
        depth: usize,
    ) -> Self {
        Self {
            inner,
            context,
            depth,
        }
    }

    /// Records one budget result after all temporary context borrows end.
    fn record<E>(
        &self,
        result: Result<(), MeasuredBudgetError<R, Q>>,
    ) -> Result<(), E>
    where
        E: Error,
    {
        self.context.borrow_mut().record(result)
    }

    /// Charges one scalar or container node at the current depth.
    fn node<E>(&self) -> Result<(), E>
    where
        E: Error,
    {
        let result = self
            .context
            .borrow_mut()
            .budget
            .enter_node_usize(self.depth);
        self.record(result)
    }

    /// Charges one string node and its UTF-8 payload length.
    fn string<E>(&self, bytes: usize) -> Result<(), E>
    where
        E: Error,
    {
        self.node()?;
        let result = self
            .context
            .borrow_mut()
            .budget
            .consume_string_bytes_usize(bytes);
        self.record(result)
    }

    /// Charges one number node and its emitted textual length.
    fn number<E>(&self, bytes: usize) -> Result<(), E>
    where
        E: Error,
    {
        self.node()?;
        let result = self
            .context
            .borrow_mut()
            .budget
            .consume_number_bytes_usize(bytes);
        self.record(result)
    }

    /// Charges one known-length array before its serializer is created.
    fn array<E>(&self, depth: usize, items: usize) -> Result<(), E>
    where
        E: Error,
    {
        let result = self
            .context
            .borrow_mut()
            .budget
            .enter_array_usize(depth, items);
        self.record(result)
    }

    /// Charges one known-length object before its serializer is created.
    fn object<E>(&self, depth: usize, entries: usize) -> Result<(), E>
    where
        E: Error,
    {
        let result = self
            .context
            .borrow_mut()
            .budget
            .enter_object_usize(depth, entries);
        self.record(result)
    }

    /// Checks and consumes one object key's UTF-8 payload length.
    fn key<E>(&self, key: &str) -> Result<(), E>
    where
        E: Error,
    {
        let result = self
            .context
            .borrow_mut()
            .budget
            .consume_key_bytes_usize(key.len());
        self.record(result)
    }
}

macro_rules! serialize_integer {
    ($name:ident, $type:ty) => {
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            self.number(value.to_string().len())?;
            self.inner.$name(value)
        }
    };
}

impl<'a, 'context, S, R, Q> Serializer
    for JsonEncodeSerializer<'a, 'context, S, R, Q>
where
    S: Serializer,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = JsonEncodeCompound<'a, 'context, S::SerializeSeq, R, Q>;
    type SerializeTuple =
        JsonEncodeCompound<'a, 'context, S::SerializeTuple, R, Q>;
    type SerializeTupleStruct =
        JsonEncodeCompound<'a, 'context, S::SerializeTupleStruct, R, Q>;
    type SerializeTupleVariant =
        JsonEncodeCompound<'a, 'context, S::SerializeTupleVariant, R, Q>;
    type SerializeMap = JsonEncodeCompound<'a, 'context, S::SerializeMap, R, Q>;
    type SerializeStruct =
        JsonEncodeCompound<'a, 'context, S::SerializeStruct, R, Q>;
    type SerializeStructVariant =
        JsonEncodeCompound<'a, 'context, S::SerializeStructVariant, R, Q>;

    /// Charges and delegates one JSON boolean.
    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        self.node()?;
        self.inner.serialize_bool(value)
    }

    serialize_integer!(serialize_i8, i8);
    serialize_integer!(serialize_i16, i16);
    serialize_integer!(serialize_i32, i32);
    serialize_integer!(serialize_i64, i64);
    serialize_integer!(serialize_i128, i128);
    serialize_integer!(serialize_u8, u8);
    serialize_integer!(serialize_u16, u16);
    serialize_integer!(serialize_u32, u32);
    serialize_integer!(serialize_u64, u64);
    serialize_integer!(serialize_u128, u128);

    /// Charges and delegates one floating-point number or JSON null.
    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        if value.is_finite() {
            let bytes = serde_json::to_string(&value)
                .map_err(S::Error::custom)?
                .len();
            self.number(bytes)?;
        } else {
            self.node()?;
        }
        self.inner.serialize_f32(value)
    }

    /// Charges and delegates one floating-point number or JSON null.
    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        if value.is_finite() {
            let bytes = serde_json::to_string(&value)
                .map_err(S::Error::custom)?
                .len();
            self.number(bytes)?;
        } else {
            self.node()?;
        }
        self.inner.serialize_f64(value)
    }

    /// Charges a character as one JSON string and delegates it.
    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.string(value.len_utf8())?;
        self.inner.serialize_char(value)
    }

    /// Charges one JSON string before delegating it.
    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        self.string(value.len())?;
        self.inner.serialize_str(value)
    }

    /// Charges the JSON byte-array structure before delegating it.
    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.array(self.depth, value.len())?;
        let child_depth = self.depth.saturating_add(1);
        for byte in value {
            let result = self
                .context
                .borrow_mut()
                .budget
                .enter_node_usize(child_depth);
            self.record(result)?;
            let result = self
                .context
                .borrow_mut()
                .budget
                .consume_number_bytes_usize(byte.to_string().len());
            self.record(result)?;
        }
        self.inner.serialize_bytes(value)
    }

    /// Charges and delegates one JSON null.
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.node()?;
        self.inner.serialize_none()
    }

    /// Delegates a present option transparently through the same depth.
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let value = BudgetedValue::new(value, self.context, self.depth);
        self.inner.serialize_some(&value)
    }

    /// Charges and delegates one JSON null.
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.node()?;
        self.inner.serialize_unit()
    }

    /// Charges a unit struct as one JSON null.
    fn serialize_unit_struct(
        self,
        name: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.node()?;
        self.inner.serialize_unit_struct(name)
    }

    /// Charges a unit variant as one JSON string.
    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.string(variant.len())?;
        self.inner
            .serialize_unit_variant(name, variant_index, variant)
    }

    /// Delegates a newtype struct transparently through the same depth.
    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let value = BudgetedValue::new(value, self.context, self.depth);
        self.inner.serialize_newtype_struct(name, &value)
    }

    /// Charges a newtype variant's outer object and decorates its payload.
    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.object(self.depth, 1)?;
        self.key(variant)?;
        let value = BudgetedValue::new(
            value,
            self.context,
            self.depth.saturating_add(1),
        );
        self.inner.serialize_newtype_variant(
            name,
            variant_index,
            variant,
            &value,
        )
    }

    /// Charges an array before asking the inner serializer to create it.
    fn serialize_seq(
        self,
        len: Option<usize>,
    ) -> Result<Self::SerializeSeq, Self::Error> {
        self.node()?;
        let context = self.context;
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_seq(len)?;
        Ok(JsonEncodeCompound::new(inner, context, child_depth))
    }

    /// Charges a fixed-length JSON tuple array.
    fn serialize_tuple(
        self,
        len: usize,
    ) -> Result<Self::SerializeTuple, Self::Error> {
        self.node()?;
        let context = self.context;
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_tuple(len)?;
        Ok(JsonEncodeCompound::new(inner, context, child_depth))
    }

    /// Charges a fixed-length JSON tuple-struct array.
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.node()?;
        let context = self.context;
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_tuple_struct(name, len)?;
        Ok(JsonEncodeCompound::new(inner, context, child_depth))
    }

    /// Charges a tuple variant's outer object and nested array.
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.object(self.depth, 1)?;
        self.key(variant)?;
        let array_depth = self.depth.saturating_add(1);
        let result = self
            .context
            .borrow_mut()
            .budget
            .enter_node_usize(array_depth);
        self.record(result)?;
        let context = self.context;
        let child_depth = array_depth.saturating_add(1);
        let inner = self.inner.serialize_tuple_variant(
            name,
            variant_index,
            variant,
            len,
        )?;
        Ok(JsonEncodeCompound::new(inner, context, child_depth))
    }

    /// Charges an object before asking the inner serializer to create it.
    fn serialize_map(
        self,
        len: Option<usize>,
    ) -> Result<Self::SerializeMap, Self::Error> {
        if let Some(entries) = len {
            let result = self
                .context
                .borrow()
                .budget
                .check_map_entries_usize(entries);
            self.record(result)?;
        }
        self.node()?;
        let context = self.context;
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_map(len)?;
        Ok(JsonEncodeCompound::new(inner, context, child_depth))
    }

    /// Charges a JSON object or recognizes serde_json's private number shape.
    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        match classify_private_struct(name) {
            Some(PrivateStructKind::Number) => {
                self.node()?;
                let context = self.context;
                let depth = self.depth;
                let inner = self.inner.serialize_struct(name, len)?;
                Ok(JsonEncodeCompound::number(inner, context, depth))
            }
            Some(PrivateStructKind::RawValue) => {
                let context = self.context;
                let depth = self.depth;
                let inner = self.inner.serialize_struct(name, len)?;
                Ok(JsonEncodeCompound::raw_value(inner, context, depth))
            }
            None => {
                self.node()?;
                let context = self.context;
                let child_depth = self.depth.saturating_add(1);
                let inner = self.inner.serialize_struct(name, len)?;
                Ok(JsonEncodeCompound::new(inner, context, child_depth))
            }
        }
    }

    /// Charges a struct variant's outer and inner objects.
    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.object(self.depth, 1)?;
        self.key(variant)?;
        let object_depth = self.depth.saturating_add(1);
        let result = self
            .context
            .borrow_mut()
            .budget
            .enter_node_usize(object_depth);
        self.record(result)?;
        let context = self.context;
        let child_depth = object_depth.saturating_add(1);
        let inner = self.inner.serialize_struct_variant(
            name,
            variant_index,
            variant,
            len,
        )?;
        Ok(JsonEncodeCompound::new(inner, context, child_depth))
    }

    /// Formats one display value once, checks it as a string, and emits it.
    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display + ?Sized,
    {
        self.node()?;
        let text = collect_display::<S::Error, _, _, Q>(
            value,
            self.context,
            DisplayBudgetKind::String,
        )?;
        self.inner.serialize_str(&text)
    }

    /// Preserves the underlying serializer's readability contract.
    #[inline(always)]
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}
