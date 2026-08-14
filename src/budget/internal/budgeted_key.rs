// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serde serializer decorator for JSON object keys.
// qubit-style: allow source-test-pair
// qubit-style: allow multiple-public-types
// qubit-style: allow explicit-imports

use std::cell::RefCell;
use std::fmt::Display;

use qubit_budget::ResourceQuantity;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error;

use super::display_budget_kind::DisplayBudgetKind;
use super::json_encode_context::JsonEncodeContext;
use super::json_encode_context::collect_display;

/// Wraps a map key so it is traversed once through a key-aware decorator.
pub(super) struct BudgetedKey<'a, 'budget, 'context, T, R, Q>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Original map key.
    value: &'a T,

    /// Shared traversal context.
    context: &'context RefCell<JsonEncodeContext<'budget, R, Q>>,
}

impl<'a, 'budget, 'context, T, R, Q> BudgetedKey<'a, 'budget, 'context, T, R, Q>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Creates a key wrapper bound to the shared traversal context.
    pub(super) const fn new(
        value: &'a T,
        context: &'context RefCell<JsonEncodeContext<'budget, R, Q>>,
    ) -> Self {
        Self { value, context }
    }
}

impl<T, R, Q> Serialize for BudgetedKey<'_, '_, '_, T, R, Q>
where
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Serializes the original key once through a key-aware decorator.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value.serialize(JsonKeyBudgetSerializer {
            inner: serializer,
            context: self.context,
        })
    }
}

/// Decorates serde_json's map-key serializer with key-byte checks.
struct JsonKeyBudgetSerializer<'context, 'budget, S, R, Q>
where
    Q: ResourceQuantity,
{
    /// Underlying map-key serializer.
    inner: S,

    /// Shared traversal context.
    context: &'context RefCell<JsonEncodeContext<'budget, R, Q>>,
}

impl<S, R, Q> JsonKeyBudgetSerializer<'_, '_, S, R, Q>
where
    S: Serializer,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Checks and consumes one emitted key length, retaining any original
    /// error.
    fn check(&self, bytes: usize) -> Result<(), S::Error> {
        let result = self
            .context
            .borrow_mut()
            .budget
            .consume_key_bytes_usize(bytes);
        self.context.borrow_mut().record(result)
    }
}

macro_rules! serialize_key_number {
    ($name:ident, $type:ty) => {
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            self.check(value.to_string().len())?;
            self.inner.$name(value)
        }
    };
}

impl<'context, 'budget, S, R, Q> Serializer
    for JsonKeyBudgetSerializer<'context, 'budget, S, R, Q>
where
    S: Serializer,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = S::SerializeSeq;
    type SerializeTuple = S::SerializeTuple;
    type SerializeTupleStruct = S::SerializeTupleStruct;
    type SerializeTupleVariant = S::SerializeTupleVariant;
    type SerializeMap = S::SerializeMap;
    type SerializeStruct = S::SerializeStruct;
    type SerializeStructVariant = S::SerializeStructVariant;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        self.check(if value { 4 } else { 5 })?;
        self.inner.serialize_bool(value)
    }

    serialize_key_number!(serialize_i8, i8);
    serialize_key_number!(serialize_i16, i16);
    serialize_key_number!(serialize_i32, i32);
    serialize_key_number!(serialize_i64, i64);
    serialize_key_number!(serialize_i128, i128);
    serialize_key_number!(serialize_u8, u8);
    serialize_key_number!(serialize_u16, u16);
    serialize_key_number!(serialize_u32, u32);
    serialize_key_number!(serialize_u64, u64);
    serialize_key_number!(serialize_u128, u128);

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        if value.is_finite() {
            let bytes = serde_json::to_string(&value)
                .map_err(S::Error::custom)?
                .len();
            self.check(bytes)?;
        }
        self.inner.serialize_f32(value)
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        if value.is_finite() {
            let bytes = serde_json::to_string(&value)
                .map_err(S::Error::custom)?
                .len();
            self.check(bytes)?;
        }
        self.inner.serialize_f64(value)
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.check(value.len_utf8())?;
        self.inner.serialize_char(value)
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        self.check(value.len())?;
        self.inner.serialize_str(value)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_bytes(value)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_none()
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit()
    }

    fn serialize_unit_struct(
        self,
        name: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit_struct(name)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.check(variant.len())?;
        self.inner
            .serialize_unit_variant(name, variant_index, variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

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
        self.inner.serialize_newtype_variant(
            name,
            variant_index,
            variant,
            value,
        )
    }

    fn serialize_seq(
        self,
        len: Option<usize>,
    ) -> Result<Self::SerializeSeq, Self::Error> {
        self.inner.serialize_seq(len)
    }

    fn serialize_tuple(
        self,
        len: usize,
    ) -> Result<Self::SerializeTuple, Self::Error> {
        self.inner.serialize_tuple(len)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.inner.serialize_tuple_struct(name, len)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.inner
            .serialize_tuple_variant(name, variant_index, variant, len)
    }

    fn serialize_map(
        self,
        len: Option<usize>,
    ) -> Result<Self::SerializeMap, Self::Error> {
        self.inner.serialize_map(len)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.inner.serialize_struct(name, len)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.inner
            .serialize_struct_variant(name, variant_index, variant, len)
    }

    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display + ?Sized,
    {
        let text = collect_display::<S::Error, _, _, Q>(
            value,
            self.context,
            DisplayBudgetKind::Key,
        )?;
        self.inner.serialize_str(&text)
    }

    #[inline(always)]
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}
