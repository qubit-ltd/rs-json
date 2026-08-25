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

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonContainerKind;
use qubit_budget::json::JsonMeasurement;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error;

use super::super::serde_compat::PrivateStructKind;
use super::super::serde_compat::SerdeJsonCompat;
use super::budgeted_value::BudgetedValue;
use super::display_budget_kind::DisplayBudgetKind;
use super::json_encode_compound::JsonEncodeCompound;
use super::json_encode_context::JsonEncodeContext;
use crate::internal::JsonLexemeLength;

/// Decorates one Serde serializer with eager JSON budget checks.
pub(in crate::encode) struct JsonEncodeSerializer<'transaction, 'budget, 'context, S, R, Q>
where
    Q: ResourceQuantity,
{
    /// Underlying serializer that emits JSON events.
    inner: S,

    /// Shared mutable state for this traversal.
    context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,

    /// Root-inclusive depth assigned to the current value.
    depth: usize,

    /// Whether value accounting can reject any emitted event.
    has_value_limits: bool,
}

impl<'transaction, 'budget, 'context, S, R, Q> JsonEncodeSerializer<'transaction, 'budget, 'context, S, R, Q>
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
    #[inline]
    pub(in crate::encode) fn new(
        inner: S,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
        has_value_limits: bool,
    ) -> Self {
        Self {
            inner,
            context,
            depth: 1,
            has_value_limits,
        }
    }

    /// Creates a nested decorator sharing an existing traversal context.
    pub(super) const fn with_context(
        inner: S,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
        depth: usize,
        has_value_limits: bool,
    ) -> Self {
        Self {
            inner,
            context,
            depth,
            has_value_limits,
        }
    }

    /// Stages one complete JSON measurement.
    fn admit<E>(&self, measurement: JsonMeasurement) -> Result<(), E>
    where
        E: Error,
    {
        if !self.has_value_limits {
            return Ok(());
        }
        self.context.borrow_mut().admit(measurement)
    }

    /// Enters one container only when value accounting is active.
    fn enter_container<E>(&self, kind: JsonContainerKind, depth: usize) -> Result<(), E>
    where
        E: Error,
    {
        if !self.has_value_limits {
            return Ok(());
        }
        self.context.borrow_mut().enter_container(kind, depth)
    }

    /// Charges one string node and its UTF-8 payload length.
    fn string<E>(&self, bytes: usize) -> Result<(), E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::String {
            depth: self.depth,
            bytes,
        })
    }

    /// Charges one number node and its emitted textual length.
    fn number<E>(&self, bytes: usize) -> Result<(), E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Number {
            depth: self.depth,
            bytes,
        })
    }

    /// Charges one known-length array before its serializer is created.
    fn array<E>(&self, depth: usize, items: usize) -> Result<(), E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Array { depth, items })
    }

    /// Charges one known-length object before its serializer is created.
    fn object<E>(&self, depth: usize, entries: usize) -> Result<(), E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Object { depth, entries })
    }

    /// Checks and consumes one object key's UTF-8 payload length.
    fn key<E>(&self, key: &str) -> Result<(), E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Key { bytes: key.len() })
    }
}

macro_rules! serialize_integer {
    ($name:ident, signed $type:ty) => {
        #[doc = concat!("Charges and delegates one signed `", stringify!($type), "` JSON integer.")]
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            if self.has_value_limits {
                self.number(JsonLexemeLength::signed_integer(value.into()))?;
            }
            self.inner.$name(value)
        }
    };
    ($name:ident, unsigned $type:ty) => {
        #[doc = concat!("Charges and delegates one unsigned `", stringify!($type), "` JSON integer.")]
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            if self.has_value_limits {
                self.number(JsonLexemeLength::unsigned_integer(value.into()))?;
            }
            self.inner.$name(value)
        }
    };
}

impl<'transaction, 'budget, 'context, S, R, Q> Serializer
    for JsonEncodeSerializer<'transaction, 'budget, 'context, S, R, Q>
where
    S: Serializer,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = JsonEncodeCompound<'transaction, 'budget, 'context, S::SerializeSeq, R, Q>;
    type SerializeTuple = JsonEncodeCompound<'transaction, 'budget, 'context, S::SerializeTuple, R, Q>;
    type SerializeTupleStruct = JsonEncodeCompound<'transaction, 'budget, 'context, S::SerializeTupleStruct, R, Q>;
    type SerializeTupleVariant = JsonEncodeCompound<'transaction, 'budget, 'context, S::SerializeTupleVariant, R, Q>;
    type SerializeMap = JsonEncodeCompound<'transaction, 'budget, 'context, S::SerializeMap, R, Q>;
    type SerializeStruct = JsonEncodeCompound<'transaction, 'budget, 'context, S::SerializeStruct, R, Q>;
    type SerializeStructVariant = JsonEncodeCompound<'transaction, 'budget, 'context, S::SerializeStructVariant, R, Q>;

    /// Charges and delegates one JSON boolean.
    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::Boolean { depth: self.depth })?;
        self.inner.serialize_bool(value)
    }

    serialize_integer!(serialize_i8, signed i8);
    serialize_integer!(serialize_i16, signed i16);
    serialize_integer!(serialize_i32, signed i32);
    serialize_integer!(serialize_i64, signed i64);
    serialize_integer!(serialize_u8, unsigned u8);
    serialize_integer!(serialize_u16, unsigned u16);
    serialize_integer!(serialize_u32, unsigned u32);
    serialize_integer!(serialize_u64, unsigned u64);

    /// Charges and delegates an `i128` representable as `i64` or `u64`.
    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        let bytes = if let Ok(value) = i64::try_from(value) {
            JsonLexemeLength::signed_integer(value.into())
        } else if let Ok(value) = u64::try_from(value) {
            JsonLexemeLength::unsigned_integer(value.into())
        } else {
            return Err(Self::Error::custom(
                "JSON integer is outside the supported 64-bit range",
            ));
        };
        if self.has_value_limits {
            self.number(bytes)?;
        }
        self.inner.serialize_i128(value)
    }

    /// Charges and delegates a `u128` representable as `u64`.
    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        let value64 = u64::try_from(value)
            .map_err(|_| Self::Error::custom("JSON integer is outside the supported 64-bit range"))?;
        if self.has_value_limits {
            self.number(JsonLexemeLength::unsigned_integer(value64.into()))?;
        }
        self.inner.serialize_u128(value)
    }

    /// Charges and delegates one finite floating-point number.
    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        if !value.is_finite() {
            return Err(Self::Error::custom("JSON floating-point value must be finite"));
        }
        if self.has_value_limits {
            self.number(JsonLexemeLength::finite_f32(value))?;
        }
        self.inner.serialize_f32(value)
    }

    /// Charges and delegates one finite floating-point number.
    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        if !value.is_finite() {
            return Err(Self::Error::custom("JSON floating-point value must be finite"));
        }
        if self.has_value_limits {
            self.number(JsonLexemeLength::finite_f64(value))?;
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
        if !self.has_value_limits {
            return self.inner.serialize_bytes(value);
        }
        self.array(self.depth, value.len())?;
        let child_depth = self.depth.saturating_add(1);
        for byte in value {
            self.admit(JsonMeasurement::Number {
                depth: child_depth,
                bytes: JsonLexemeLength::byte(*byte),
            })?;
        }
        self.inner.serialize_bytes(value)
    }

    /// Charges and delegates one JSON null.
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
        self.inner.serialize_none()
    }

    /// Delegates a present option transparently through the same depth.
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let value = BudgetedValue::new(value, self.context, self.depth, self.has_value_limits);
        self.inner.serialize_some(&value)
    }

    /// Charges and delegates one JSON null.
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
        self.inner.serialize_unit()
    }

    /// Charges a unit struct as one JSON null.
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
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
        self.inner.serialize_unit_variant(name, variant_index, variant)
    }

    /// Delegates a newtype struct transparently through the same depth.
    fn serialize_newtype_struct<T>(self, name: &'static str, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let value = BudgetedValue::new(value, self.context, self.depth, self.has_value_limits);
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
        let value = BudgetedValue::new(value, self.context, self.depth.saturating_add(1), self.has_value_limits);
        self.inner
            .serialize_newtype_variant(name, variant_index, variant, &value)
    }

    /// Charges an array before asking the inner serializer to create it.
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        let context = self.context;
        self.enter_container(JsonContainerKind::Sequence, self.depth)?;
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_seq(len)?;
        Ok(JsonEncodeCompound::new(
            inner,
            context,
            child_depth,
            self.has_value_limits,
        ))
    }

    /// Charges a fixed-length JSON tuple array.
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        let context = self.context;
        self.enter_container(JsonContainerKind::Sequence, self.depth)?;
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_tuple(len)?;
        Ok(JsonEncodeCompound::new(
            inner,
            context,
            child_depth,
            self.has_value_limits,
        ))
    }

    /// Charges a fixed-length JSON tuple-struct array.
    fn serialize_tuple_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        let context = self.context;
        self.enter_container(JsonContainerKind::Sequence, self.depth)?;
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_tuple_struct(name, len)?;
        Ok(JsonEncodeCompound::new(
            inner,
            context,
            child_depth,
            self.has_value_limits,
        ))
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
        self.enter_container(JsonContainerKind::Sequence, array_depth)?;
        let context = self.context;
        let child_depth = array_depth.saturating_add(1);
        let inner = self.inner.serialize_tuple_variant(name, variant_index, variant, len)?;
        Ok(JsonEncodeCompound::new(
            inner,
            context,
            child_depth,
            self.has_value_limits,
        ))
    }

    /// Charges an object before asking the inner serializer to create it.
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        let context = self.context;
        self.enter_container(JsonContainerKind::Map, self.depth)?;
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_map(len)?;
        Ok(JsonEncodeCompound::new(
            inner,
            context,
            child_depth,
            self.has_value_limits,
        ))
    }

    /// Charges a JSON object or recognizes serde_json's RawValue shape.
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        match SerdeJsonCompat::classify_private_struct(name) {
            Some(PrivateStructKind::RawValue) => {
                let context = self.context;
                let depth = self.depth;
                let inner = self.inner.serialize_struct(name, len)?;
                Ok(JsonEncodeCompound::raw_value(inner, context, depth))
            }
            None => {
                let context = self.context;
                self.enter_container(JsonContainerKind::Map, self.depth)?;
                let child_depth = self.depth.saturating_add(1);
                let inner = self.inner.serialize_struct(name, len)?;
                Ok(JsonEncodeCompound::new(
                    inner,
                    context,
                    child_depth,
                    self.has_value_limits,
                ))
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
        self.enter_container(JsonContainerKind::Map, object_depth)?;
        let context = self.context;
        let child_depth = object_depth.saturating_add(1);
        let inner = self.inner.serialize_struct_variant(name, variant_index, variant, len)?;
        Ok(JsonEncodeCompound::new(
            inner,
            context,
            child_depth,
            self.has_value_limits,
        ))
    }

    /// Formats one display value once, checks it as a string, and emits it.
    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display + ?Sized,
    {
        if !self.has_value_limits {
            return self.inner.collect_str(value);
        }
        let text = JsonEncodeContext::collect_display::<S::Error, _>(
            self.context,
            value,
            DisplayBudgetKind::String,
            self.depth,
        )?;
        self.inner.serialize_str(&text)
    }

    /// Preserves the underlying serializer's readability contract.
    #[inline(always)]
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}
