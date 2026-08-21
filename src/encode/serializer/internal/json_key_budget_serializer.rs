// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the serializer decorator for JSON object keys.
// qubit-style: allow explicit-imports

use std::cell::RefCell;
use std::fmt::Display;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonMeasurement;
use serde::Serialize;
use serde::Serializer;

use super::super::display_budget_kind::DisplayBudgetKind;
use super::super::json_encode_context::JsonEncodeContext;
use super::super::json_lexeme_length::JsonLexemeLength;

/// Decorates serde_json's map-key serializer with key-byte checks.
pub(in crate::encode::serializer) struct JsonKeyBudgetSerializer<'context, 'transaction, 'budget, S, R, Q>
where
    Q: ResourceQuantity,
{
    /// Underlying map-key serializer.
    pub(in crate::encode::serializer) inner: S,

    /// Shared traversal context.
    pub(in crate::encode::serializer) context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
}

impl<S, R, Q> JsonKeyBudgetSerializer<'_, '_, '_, S, R, Q>
where
    S: Serializer,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Checks and consumes one emitted key length, retaining any original
    /// error.
    fn check(&self, bytes: usize) -> Result<(), S::Error> {
        self.context.borrow_mut().admit(JsonMeasurement::Key { bytes })
    }
}

macro_rules! serialize_key_number {
    ($name:ident, signed $type:ty) => {
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            self.check(JsonLexemeLength::signed_integer(value.into()))?;
            self.inner.$name(value)
        }
    };
    ($name:ident, unsigned $type:ty) => {
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            self.check(JsonLexemeLength::unsigned_integer(value.into()))?;
            self.inner.$name(value)
        }
    };
}

impl<'context, 'transaction, 'budget, S, R, Q> Serializer
    for JsonKeyBudgetSerializer<'context, 'transaction, 'budget, S, R, Q>
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

    serialize_key_number!(serialize_i8, signed i8);
    serialize_key_number!(serialize_i16, signed i16);
    serialize_key_number!(serialize_i32, signed i32);
    serialize_key_number!(serialize_i64, signed i64);
    serialize_key_number!(serialize_i128, signed i128);
    serialize_key_number!(serialize_u8, unsigned u8);
    serialize_key_number!(serialize_u16, unsigned u16);
    serialize_key_number!(serialize_u32, unsigned u32);
    serialize_key_number!(serialize_u64, unsigned u64);
    serialize_key_number!(serialize_u128, unsigned u128);

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        if value.is_finite() {
            self.check(JsonLexemeLength::finite_f32(value))?;
        }
        self.inner.serialize_f32(value)
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        if value.is_finite() {
            self.check(JsonLexemeLength::finite_f64(value))?;
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

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit_struct(name)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.check(variant.len())?;
        self.inner.serialize_unit_variant(name, variant_index, variant)
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Self::Ok, Self::Error>
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
        self.inner
            .serialize_newtype_variant(name, variant_index, variant, value)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.inner.serialize_seq(len)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.inner.serialize_tuple(len)
    }

    fn serialize_tuple_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.inner.serialize_tuple_struct(name, len)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.inner.serialize_tuple_variant(name, variant_index, variant, len)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.inner.serialize_map(len)
    }

    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        self.inner.serialize_struct(name, len)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.inner.serialize_struct_variant(name, variant_index, variant, len)
    }

    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display + ?Sized,
    {
        let text = JsonEncodeContext::collect_display::<S::Error, _>(self.context, value, DisplayBudgetKind::Key, 1)?;
        self.inner.serialize_str(&text)
    }

    #[inline(always)]
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}
