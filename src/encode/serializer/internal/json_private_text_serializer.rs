// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the serializer decorator for serde_json private text payloads.

use std::cell::RefCell;
use std::fmt::Display;

use qubit_budget::ResourceQuantity;
use serde::Serialize;
use serde::Serializer;

use super::DisplayBudgetKind;
use super::JsonEncodeContext;
use super::PrivateTextKind;
use super::json_private_number_preflight;
use crate::encode::JsonSerializationErrorKind;
use crate::encode::JsonSerializerStateError;

/// Enforces serde_json's private text protocol while preserving the wrapped
/// serializer's behavior for accepted payloads.
///
/// # Type Parameters
///
/// * `'context` - Lifetime of the shared encoding context borrow.
/// * `'transaction` - Lifetime of the value transaction borrowed by the
///   context.
/// * `'budget` - Lifetime of the resource budget used by the transaction.
/// * `S` - Wrapped serializer receiving accepted payloads.
/// * `R` - Resource identifier stored by the encoding context.
/// * `Q` - Quantity representation used by resource accounting.
///
/// # Panics
///
/// A serializer callback can panic if the shared context or its output
/// accounting cell is already borrowed when the callback needs access.
pub(in crate::encode::serializer) struct JsonPrivateTextSerializer<'context, 'transaction, 'budget, S, R, Q>
where
    Q: ResourceQuantity,
{
    /// serde_json private string emitter.
    pub(in crate::encode::serializer) inner: S,

    /// Shared traversal context.
    pub(in crate::encode::serializer) context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,

    /// Budget semantics represented by the emitted text.
    pub(in crate::encode::serializer) kind: PrivateTextKind,
}

macro_rules! delegate_number_method {
    ($name:ident, $type:ty) => {
        #[doc = "Rejects this non-text payload for the selected private text protocol."]
        #[doc = "Returns the selected private protocol's stable serializer-state error."]
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            self.reject_non_text_payload()?;
            self.inner.$name(value)
        }
    };
}

impl<S, R, Q> JsonPrivateTextSerializer<'_, '_, '_, S, R, Q>
where
    S: Serializer,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Rejects non-text payload methods for private text protocols.
    ///
    /// Returns an error when the serializer represents a private text value,
    /// whose protocol accepts string payloads only.
    ///
    /// # Returns
    ///
    /// `Err` containing the selected private protocol's stable state error.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error carrying
    /// `InvalidNumberProtocol` or `InvalidRawValueProtocol` when a non-text
    /// payload is rejected.
    ///
    /// # Panics
    ///
    /// Panics if the shared context is already borrowed.
    fn reject_non_text_payload(&self) -> Result<(), S::Error> {
        let reason = match self.kind {
            PrivateTextKind::RawValue { .. } => JsonSerializerStateError::InvalidRawValueProtocol,
            PrivateTextKind::Number { .. } => JsonSerializerStateError::InvalidNumberProtocol,
        };
        Err(self
            .context
            .borrow_mut()
            .serialization_error(JsonSerializationErrorKind::InvalidSerializerState { reason }))
    }
}

impl<'context, 'transaction, 'budget, S, R, Q> Serializer
    for JsonPrivateTextSerializer<'context, 'transaction, 'budget, S, R, Q>
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

    delegate_number_method!(serialize_bool, bool);
    delegate_number_method!(serialize_i8, i8);
    delegate_number_method!(serialize_i16, i16);
    delegate_number_method!(serialize_i32, i32);
    delegate_number_method!(serialize_i64, i64);
    delegate_number_method!(serialize_i128, i128);
    delegate_number_method!(serialize_u8, u8);
    delegate_number_method!(serialize_u16, u16);
    delegate_number_method!(serialize_u32, u32);
    delegate_number_method!(serialize_u64, u64);
    delegate_number_method!(serialize_u128, u128);
    delegate_number_method!(serialize_f32, f32);
    delegate_number_method!(serialize_f64, f64);
    delegate_number_method!(serialize_char, char);

    /// Preflights raw JSON or number text according to the selected protocol
    /// before forwarding it.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `value` - Text payload checked against the selected private protocol.
    ///
    /// # Returns
    ///
    /// The wrapped serializer's result after protocol preflight succeeds.
    ///
    /// # Errors
    ///
    /// Returns a preflight or wrapped serializer error when the text is
    /// invalid for the selected protocol or cannot be serialized.
    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        match self.kind {
            PrivateTextKind::RawValue { depth } => {
                self.context.borrow_mut().preflight_raw(value, depth)?;
            }
            PrivateTextKind::Number { depth } => {
                json_private_number_preflight::preflight(self.context, value, depth)?;
            }
        }
        self.inner.serialize_str(value)
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.reject_non_text_payload()?;
        self.inner.serialize_bytes(value)
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.reject_non_text_payload()?;
        self.inner.serialize_none()
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.reject_non_text_payload()?;
        self.inner.serialize_some(value)
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.reject_non_text_payload()?;
        self.inner.serialize_unit()
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.reject_non_text_payload()?;
        self.inner.serialize_unit_struct(name)
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.reject_non_text_payload()?;
        self.inner.serialize_unit_variant(name, variant_index, variant)
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_newtype_struct<T>(self, name: &'static str, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.reject_non_text_payload()?;
        self.inner.serialize_newtype_struct(name, value)
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
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
        self.reject_non_text_payload()?;
        self.inner
            .serialize_newtype_variant(name, variant_index, variant, value)
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.reject_non_text_payload()?;
        self.inner.serialize_seq(len)
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.reject_non_text_payload()?;
        self.inner.serialize_tuple(len)
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_tuple_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.reject_non_text_payload()?;
        self.inner.serialize_tuple_struct(name, len)
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.reject_non_text_payload()?;
        self.inner.serialize_tuple_variant(name, variant_index, variant, len)
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.reject_non_text_payload()?;
        self.inner.serialize_map(len)
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        self.reject_non_text_payload()?;
        self.inner.serialize_struct(name, len)
    }

    /// Rejects this non-text payload because the selected private text
    /// protocol accepts string payloads only.
    ///
    /// # Errors
    ///
    /// Returns the selected protocol's stable serializer-state error.
    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.reject_non_text_payload()?;
        self.inner.serialize_struct_variant(name, variant_index, variant, len)
    }

    /// Formats a display value, preflights it according to the selected
    /// protocol, then forwards it as text.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Displayable value type.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `value` - Value formatted, preflighted, and forwarded as text.
    ///
    /// # Returns
    ///
    /// The wrapped serializer's result after preflight succeeds.
    ///
    /// # Errors
    ///
    /// Returns a formatting, preflight, or wrapped serializer error.
    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display + ?Sized,
    {
        let (depth, budget_kind) = match self.kind {
            PrivateTextKind::RawValue { depth } => (depth, DisplayBudgetKind::RawOutput),
            PrivateTextKind::Number { depth } => (depth, DisplayBudgetKind::Number),
        };
        let text = JsonEncodeContext::collect_display::<S::Error, _>(self.context, value, budget_kind, depth)?;
        match self.kind {
            PrivateTextKind::RawValue { .. } => {
                self.context.borrow_mut().preflight_raw(&text, depth)?;
            }
            PrivateTextKind::Number { .. } => {
                json_private_number_preflight::preflight(self.context, &text, depth)?;
            }
        }
        self.inner.serialize_str(&text)
    }

    /// Reports the underlying serializer's human-readable mode.
    ///
    /// # Returns
    ///
    /// The human-readable mode reported by the wrapped serializer.
    #[inline(always)]
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}
