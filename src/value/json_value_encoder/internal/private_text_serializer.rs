// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serializer for text carried by serde_json private protocols.

use std::fmt;

use serde::Serialize;
use serde::Serializer;
use serde::ser::Impossible;
use serde_json::Value;

use super::json_value_compound::private_invalid_state;
use super::private_text_collector::PrivateTextCollector;
use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;
use crate::internal::JsonPrivateProtocolKind;

/// Serializer that accepts only string and Display based private payloads.
pub(super) struct PrivateTextSerializer<'a> {
    /// Selects which serde_json private protocol the text belongs to.
    pub(super) kind: JsonPrivateProtocolKind,
    /// Checks prospective private payload lengths against the projection context.
    pub(super) check_text_bytes: &'a mut dyn FnMut(usize) -> Result<(), JsonSerializationError>,
}

impl Serializer for PrivateTextSerializer<'_> {
    /// JSON value produced by an accepted text payload.
    type Ok = Value;
    /// Stable error reported when the private payload has an unsupported shape.
    type Error = JsonSerializationError;
    /// Sequence payloads are rejected before a compound serializer is created.
    type SerializeSeq = Impossible<Value, JsonSerializationError>;
    /// Tuple payloads are rejected before a compound serializer is created.
    type SerializeTuple = Impossible<Value, JsonSerializationError>;
    /// Tuple-struct payloads are rejected before a compound serializer is
    /// created.
    type SerializeTupleStruct = Impossible<Value, JsonSerializationError>;
    /// Tuple-variant payloads are rejected before a compound serializer is
    /// created.
    type SerializeTupleVariant = Impossible<Value, JsonSerializationError>;
    /// Map payloads are rejected before a compound serializer is created.
    type SerializeMap = Impossible<Value, JsonSerializationError>;
    /// Struct payloads are rejected before a compound serializer is created.
    type SerializeStruct = Impossible<Value, JsonSerializationError>;
    /// Struct-variant payloads are rejected before a compound serializer is
    /// created.
    type SerializeStructVariant = Impossible<Value, JsonSerializationError>;

    /// Rejects boolean payloads because this private protocol requires text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_bool(self, _: bool) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects signed integer payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_i8(self, _: i8) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects signed integer payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_i16(self, _: i16) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects signed integer payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_i32(self, _: i32) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects signed integer payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_i64(self, _: i64) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects signed integer payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_i128(self, _: i128) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects unsigned integer payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_u8(self, _: u8) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects unsigned integer payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_u16(self, _: u16) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects unsigned integer payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_u32(self, _: u32) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects unsigned integer payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_u64(self, _: u64) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects unsigned integer payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_u128(self, _: u128) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects floating-point payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_f32(self, _: f32) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects floating-point payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_f64(self, _: f64) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects character payloads because this private protocol requires string
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_char(self, _: char) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }

    /// Copies accepted string payload text into a JSON string value.
    ///
    /// # Parameters
    ///
    /// * `value` - String text copied into an owned JSON string.
    ///
    /// # Returns
    ///
    /// A JSON string containing a copy of `value`.
    ///
    /// # Errors
    ///
    /// Returns a serialization carrier error if the configured private-text
    /// byte limit rejects `value`.
    fn serialize_str(self, value: &str) -> Result<Value, Self::Error> {
        (self.check_text_bytes)(value.len())?;
        Ok(Value::String(value.to_owned()))
    }

    /// Rejects byte payloads because this private protocol requires string
    /// text.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; this protocol rejects its shape.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_bytes(self, _: &[u8]) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects absent optional payloads because this private protocol requires
    /// text.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error because absent optional payloads
    /// are not text.
    fn serialize_none(self) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects wrapped optional payloads because this private protocol requires
    /// text.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Wrapped payload type.
    ///
    /// # Parameters
    ///
    /// * The payload value supplied by Serde; optional wrappers are rejected.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_some<T: Serialize + ?Sized>(self, _: &T) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects unit payloads because this private protocol requires text.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error because unit payloads are not
    /// text.
    fn serialize_unit(self) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects unit-struct payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The unit-struct name supplied by Serde; the shape is rejected.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_unit_struct(self, _: &'static str) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects unit-variant payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The enum and variant metadata supplied by Serde; unit variants are
    ///   rejected.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects newtype payloads because this private protocol requires text.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Wrapped payload type.
    ///
    /// # Parameters
    ///
    /// * The newtype metadata and wrapped value supplied by Serde; the shape is
    ///   rejected.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _: &'static str, _: &T) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects newtype-variant payloads because this private protocol requires
    /// text.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Wrapped payload type.
    ///
    /// # Parameters
    ///
    /// * The enum, variant, and wrapped value supplied by Serde; the shape is
    ///   rejected.
    ///
    /// # Returns
    ///
    /// No value is produced.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects sequence payloads because this private protocol requires text.
    ///
    /// # Parameters
    ///
    /// * The optional sequence length supplied by Serde; sequence payloads are
    ///   rejected.
    ///
    /// # Returns
    ///
    /// No value is produced; the `Impossible` associated serializer has no
    /// constructible success result.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects tuple payloads because this private protocol requires text.
    ///
    /// # Parameters
    ///
    /// * The tuple length supplied by Serde; tuple payloads are rejected.
    ///
    /// # Returns
    ///
    /// No value is produced; the `Impossible` associated serializer has no
    /// constructible success result.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects tuple-struct payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The tuple-struct name and field count supplied by Serde; this shape is
    ///   rejected.
    ///
    /// # Returns
    ///
    /// No value is produced; the `Impossible` associated serializer has no
    /// constructible success result.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects tuple-variant payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The enum, variant, and field-count metadata supplied by Serde; this
    ///   shape is rejected.
    ///
    /// # Returns
    ///
    /// No value is produced; the `Impossible` associated serializer has no
    /// constructible success result.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects map payloads because this private protocol requires text.
    ///
    /// # Parameters
    ///
    /// * The optional map length supplied by Serde; map payloads are rejected.
    ///
    /// # Returns
    ///
    /// No value is produced; the `Impossible` associated serializer has no
    /// constructible success result.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects struct payloads because this private protocol requires text.
    ///
    /// # Parameters
    ///
    /// * The struct name and field count supplied by Serde; struct payloads are
    ///   rejected.
    ///
    /// # Returns
    ///
    /// No value is produced; the `Impossible` associated serializer has no
    /// constructible success result.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    /// Rejects struct-variant payloads because this private protocol requires
    /// text.
    ///
    /// # Parameters
    ///
    /// * The enum, variant, and field-count metadata supplied by Serde; this
    ///   shape is rejected.
    ///
    /// # Returns
    ///
    /// No value is produced; the `Impossible` associated serializer has no
    /// constructible success result.
    ///
    /// # Errors
    ///
    /// Returns the stable invalid-state error for the selected private
    /// protocol.
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(private_invalid_state(self.kind))
    }

    /// Formats Display output into the string payload and reports formatting
    /// failures.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Displayable payload type.
    ///
    /// # Parameters
    ///
    /// * `value` - Value whose `Display` output becomes the text payload.
    ///
    /// # Returns
    ///
    /// A JSON string containing the formatted text.
    ///
    /// # Errors
    ///
    /// Returns the first private-text limit failure, or
    /// `DisplayFormattingFailed` if formatting rejects a write before any
    /// limit failure occurs.
    fn collect_str<T: fmt::Display + ?Sized>(self, value: &T) -> Result<Value, Self::Error> {
        let mut collector = PrivateTextCollector::new(self.check_text_bytes);
        let formatted = fmt::write(&mut collector, format_args!("{value}"));
        if let Some(error) = collector.take_failure() {
            return Err(error);
        }
        formatted.map_err(|_| JsonSerializationError::new(JsonSerializationErrorKind::DisplayFormattingFailed))?;
        let text = collector.into_string();
        Ok(Value::String(text))
    }
}
