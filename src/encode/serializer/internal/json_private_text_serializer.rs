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
        #[doc = "Forwards the scalar unchanged outside Number mode."]
        #[doc = "Number mode rejects scalar payloads; numbers must use a text method."]
        #[doc = ""]
        #[doc = "# Parameters"]
        #[doc = ""]
        #[doc = "* `self` — serializer decorator consumed for this payload."]
        #[doc = "* `value` — scalar payload forwarded to the wrapped serializer."]
        #[doc = ""]
        #[doc = "# Returns"]
        #[doc = ""]
        #[doc = "The result returned by the wrapped serializer."]
        #[doc = ""]
        #[doc = "# Errors"]
        #[doc = ""]
        #[doc = "Returns the wrapped serializer's error, or the protocol error"]
        #[doc = "when Number mode rejects this non-text payload."]
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            self.reject_non_text_number_payload()?;
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
    /// Rejects non-text payload methods for the Number protocol.
    ///
    /// Returns an error only when the serializer represents a number, whose
    /// protocol accepts textual numeric payloads only.
    ///
    /// # Returns
    ///
    /// `Ok(())` outside Number mode, or `Err` when Number mode rejects a
    /// non-text payload.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error carrying
    /// `InvalidNumberProtocol` when a non-text Number payload is rejected.
    ///
    /// # Panics
    ///
    /// Panics if the shared context is already borrowed.
    fn reject_non_text_number_payload(&self) -> Result<(), S::Error> {
        if matches!(self.kind, PrivateTextKind::Number { .. }) {
            return Err(self.context.borrow_mut().serialization_error(
                JsonSerializationErrorKind::InvalidSerializerState {
                    reason: JsonSerializerStateError::InvalidNumberProtocol,
                },
            ));
        }
        Ok(())
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

    /// Forwards byte payloads unless Number mode rejects this non-text payload.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `value` - Byte payload forwarded to the wrapped serializer.
    ///
    /// # Returns
    ///
    /// The result returned by the wrapped serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.reject_non_text_number_payload()?;
        self.inner.serialize_bytes(value)
    }

    /// Forwards an absent optional payload unless Number mode rejects it.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    ///
    /// # Returns
    ///
    /// The result returned by the wrapped serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.reject_non_text_number_payload()?;
        self.inner.serialize_none()
    }

    /// Forwards a present optional payload unless Number mode rejects it.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Serializable wrapped payload type.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `value` - Present payload forwarded to the wrapped serializer.
    ///
    /// # Returns
    ///
    /// The result returned by the wrapped serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.reject_non_text_number_payload()?;
        self.inner.serialize_some(value)
    }

    /// Forwards a unit payload unless Number mode rejects this non-text
    /// payload.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    ///
    /// # Returns
    ///
    /// The result returned by the wrapped serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.reject_non_text_number_payload()?;
        self.inner.serialize_unit()
    }

    /// Forwards a unit-struct payload unless Number mode rejects it.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `name` - Unit-struct name forwarded to the wrapped serializer.
    ///
    /// # Returns
    ///
    /// The result returned by the wrapped serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.reject_non_text_number_payload()?;
        self.inner.serialize_unit_struct(name)
    }

    /// Forwards a unit-variant payload unless Number mode rejects it.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `name` - Enum name forwarded to the wrapped serializer.
    /// * `variant_index` - Variant index forwarded to the wrapped serializer.
    /// * `variant` - Variant name forwarded to the wrapped serializer.
    ///
    /// # Returns
    ///
    /// The result returned by the wrapped serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.reject_non_text_number_payload()?;
        self.inner.serialize_unit_variant(name, variant_index, variant)
    }

    /// Forwards a newtype-struct payload unless Number mode rejects it.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Serializable wrapped payload type.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `name` - Newtype-struct name forwarded to the wrapped serializer.
    /// * `value` - Wrapped payload forwarded to the wrapped serializer.
    ///
    /// # Returns
    ///
    /// The result returned by the wrapped serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_newtype_struct<T>(self, name: &'static str, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.reject_non_text_number_payload()?;
        self.inner.serialize_newtype_struct(name, value)
    }

    /// Forwards a newtype-variant payload unless Number mode rejects it.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Serializable wrapped payload type.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `name` - Enum name forwarded to the wrapped serializer.
    /// * `variant_index` - Variant index forwarded to the wrapped serializer.
    /// * `variant` - Variant name forwarded to the wrapped serializer.
    /// * `value` - Wrapped payload forwarded to the wrapped serializer.
    ///
    /// # Returns
    ///
    /// The result returned by the wrapped serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
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
        self.reject_non_text_number_payload()?;
        self.inner
            .serialize_newtype_variant(name, variant_index, variant, value)
    }

    /// Creates a sequence serializer unless Number mode rejects this non-text
    /// payload.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `len` - Optional sequence length forwarded to the wrapped serializer.
    ///
    /// # Returns
    ///
    /// The wrapped serializer's sequence serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.reject_non_text_number_payload()?;
        self.inner.serialize_seq(len)
    }

    /// Creates a tuple serializer unless Number mode rejects this non-text
    /// payload.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `len` - Tuple length forwarded to the wrapped serializer.
    ///
    /// # Returns
    ///
    /// The wrapped serializer's tuple serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.reject_non_text_number_payload()?;
        self.inner.serialize_tuple(len)
    }

    /// Creates a tuple-struct serializer unless Number mode rejects it.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `name` - Tuple-struct name forwarded to the wrapped serializer.
    /// * `len` - Tuple-struct field count forwarded to the wrapped serializer.
    ///
    /// # Returns
    ///
    /// The wrapped serializer's tuple-struct serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_tuple_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.reject_non_text_number_payload()?;
        self.inner.serialize_tuple_struct(name, len)
    }

    /// Creates a tuple-variant serializer unless Number mode rejects it.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `name` - Enum name forwarded to the wrapped serializer.
    /// * `variant_index` - Variant index forwarded to the wrapped serializer.
    /// * `variant` - Variant name forwarded to the wrapped serializer.
    /// * `len` - Variant field count forwarded to the wrapped serializer.
    ///
    /// # Returns
    ///
    /// The wrapped serializer's tuple-variant serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.reject_non_text_number_payload()?;
        self.inner.serialize_tuple_variant(name, variant_index, variant, len)
    }

    /// Creates a map serializer unless Number mode rejects this non-text
    /// payload.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `len` - Optional entry count forwarded to the wrapped serializer.
    ///
    /// # Returns
    ///
    /// The wrapped serializer's map serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.reject_non_text_number_payload()?;
        self.inner.serialize_map(len)
    }

    /// Creates a struct serializer unless Number mode rejects this non-text
    /// payload.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `name` - Struct name forwarded to the wrapped serializer.
    /// * `len` - Struct field count forwarded to the wrapped serializer.
    ///
    /// # Returns
    ///
    /// The wrapped serializer's struct serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        self.reject_non_text_number_payload()?;
        self.inner.serialize_struct(name, len)
    }

    /// Creates a struct-variant serializer unless Number mode rejects it.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer decorator consumed for this payload.
    /// * `name` - Enum name forwarded to the wrapped serializer.
    /// * `variant_index` - Variant index forwarded to the wrapped serializer.
    /// * `variant` - Variant name forwarded to the wrapped serializer.
    /// * `len` - Variant field count forwarded to the wrapped serializer.
    ///
    /// # Returns
    ///
    /// The wrapped serializer's struct-variant serializer.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error, or the protocol error when
    /// Number mode rejects this non-text payload.
    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.reject_non_text_number_payload()?;
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
        let depth = match self.kind {
            PrivateTextKind::RawValue { depth } | PrivateTextKind::Number { depth } => depth,
        };
        let text = JsonEncodeContext::collect_display::<S::Error, _>(
            self.context,
            value,
            DisplayBudgetKind::RawOutput,
            depth,
        )?;
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
