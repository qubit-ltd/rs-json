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
use crate::internal::JsonLexemeLength;
use crate::internal::JsonMapKey;

/// Decorates serde_json's map-key serializer with key-byte checks.
pub(in crate::encode::serializer) struct JsonKeyBudgetSerializer<'context, 'transaction, 'budget, S, R, Q>
where
    Q: ResourceQuantity,
{
    /// Underlying map-key serializer.
    pub(in crate::encode::serializer) inner: S,

    /// Shared traversal context.
    pub(in crate::encode::serializer) context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,

    /// Whether key accounting can reject the emitted key.
    pub(in crate::encode::serializer) has_value_limits: bool,
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
        if !self.has_value_limits {
            return Ok(());
        }
        self.context.borrow_mut().admit(JsonMeasurement::Key { bytes })
    }
}

macro_rules! serialize_key_number {
    ($name:ident, signed $type:ty) => {
        #[doc = "Checks the serialized signed key length before forwarding it."]
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            if self.has_value_limits {
                self.check(JsonLexemeLength::signed_integer(value.into()))?;
            }
            self.inner.$name(value)
        }
    };
    ($name:ident, unsigned $type:ty) => {
        #[doc = "Checks the serialized unsigned key length before forwarding it."]
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            if self.has_value_limits {
                self.check(JsonLexemeLength::unsigned_integer(value.into()))?;
            }
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

    /// Checks the boolean key length before forwarding it.
    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        self.check(if value { 4 } else { 5 })?;
        self.inner.serialize_bool(value)
    }

    serialize_key_number!(serialize_i8, signed i8);
    serialize_key_number!(serialize_i16, signed i16);
    serialize_key_number!(serialize_i32, signed i32);
    serialize_key_number!(serialize_i64, signed i64);
    serialize_key_number!(serialize_u8, unsigned u8);
    serialize_key_number!(serialize_u16, unsigned u16);
    serialize_key_number!(serialize_u32, unsigned u32);
    serialize_key_number!(serialize_u64, unsigned u64);

    /// Charges and emits one full-range signed integer key as decimal text.
    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        let text = JsonMapKey::signed_wide(value);
        if self.has_value_limits {
            self.check(text.len())?;
        }
        self.inner.serialize_str(&text)
    }

    /// Charges and emits one full-range unsigned integer key as decimal text.
    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        let text = JsonMapKey::unsigned_wide(value);
        if self.has_value_limits {
            self.check(text.len())?;
        }
        self.inner.serialize_str(&text)
    }

    /// Checks a finite `f32` key length before forwarding it.
    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        if self.has_value_limits && value.is_finite() {
            self.check(JsonLexemeLength::finite_f32(value))?;
        }
        self.inner.serialize_f32(value)
    }

    /// Checks a finite `f64` key length before forwarding it.
    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        if self.has_value_limits && value.is_finite() {
            self.check(JsonLexemeLength::finite_f64(value))?;
        }
        self.inner.serialize_f64(value)
    }

    /// Checks the UTF-8 key length before forwarding a character.
    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        if self.has_value_limits {
            self.check(value.len_utf8())?;
        }
        self.inner.serialize_char(value)
    }

    /// Checks the string key length before forwarding it.
    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        if self.has_value_limits {
            self.check(value.len())?;
        }
        self.inner.serialize_str(value)
    }

    /// Forwards byte keys to the underlying serializer.
    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_bytes(value)
    }

    /// Forwards an absent optional key without additional accounting.
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_none()
    }

    /// Serializes a present optional key through this decorator.
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    /// Forwards a unit key without additional accounting.
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit()
    }

    /// Forwards a unit-struct key without additional accounting.
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit_struct(name)
    }

    /// Checks a unit-variant key name before forwarding it.
    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        if self.has_value_limits {
            self.check(variant.len())?;
        }
        self.inner.serialize_unit_variant(name, variant_index, variant)
    }

    /// Serializes a newtype key through this decorator.
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    /// Forwards a newtype-variant key and its payload.
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

    /// Creates a sequence serializer through the underlying serializer.
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.inner.serialize_seq(len)
    }

    /// Creates a tuple serializer through the underlying serializer.
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.inner.serialize_tuple(len)
    }

    /// Creates a tuple-struct serializer through the underlying serializer.
    fn serialize_tuple_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.inner.serialize_tuple_struct(name, len)
    }

    /// Creates a tuple-variant serializer through the underlying serializer.
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.inner.serialize_tuple_variant(name, variant_index, variant, len)
    }

    /// Creates a map serializer through the underlying serializer.
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.inner.serialize_map(len)
    }

    /// Creates a struct serializer through the underlying serializer.
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        self.inner.serialize_struct(name, len)
    }

    /// Creates a struct-variant serializer through the underlying serializer.
    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.inner.serialize_struct_variant(name, variant_index, variant, len)
    }

    /// Formats, accounts, and forwards a display-based key.
    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display + ?Sized,
    {
        if !self.has_value_limits {
            return self.inner.collect_str(value);
        }
        let text = JsonEncodeContext::collect_display::<S::Error, _>(self.context, value, DisplayBudgetKind::Key, 1)?;
        self.inner.serialize_str(&text)
    }

    /// Reports the underlying serializer's human-readable mode.
    #[inline(always)]
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}
