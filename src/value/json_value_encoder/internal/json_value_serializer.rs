// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Transaction-aware Serde serializer for strict JSON value projection.

use std::cell::RefCell;
use std::fmt::Display;
use std::fmt::Write as _;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonContainerKind;
use qubit_budget::json::JsonMeasurement;
use serde::Serialize;
use serde::Serializer;
use serde_json::Number;
use serde_json::Value;
use serde_json::to_string;

use super::JsonValueCompound;
use super::JsonValueDisplayCollector;
use super::JsonValueDisplayKind;
use super::JsonValueEncodeContext;
use crate::encode::JsonCollectionKind;
use crate::encode::JsonIntegerSignedness;
use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;
use crate::internal::JsonPrivateProtocolKind;
use crate::internal::JsonPrivateProtocolState;
use crate::value::SafeJsonValue;
use crate::value::internal::SafeValueBuilder;

/// Shared interior-mutable context used by the serializer and its descendants.
type ContextCell<'context, 'transaction, 'budget, R, Q> =
    &'context RefCell<JsonValueEncodeContext<'transaction, 'budget, R, Q>>;

/// Serializer facade for one node at its root-inclusive depth.
///
/// The root is serialized at depth one and child serializers share the same
/// context, which carries the transaction, sticky first failure, and private
/// payload limit configured by the owning encoder. The private raw-value and
/// number protocols are recognized only through their reserved struct names;
/// a generic map event sequence does not identify a private `Number`.
#[derive(Clone, Copy)]
pub(in crate::value::json_value_encoder) struct JsonValueSerializer<'context, 'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Shared transaction, failure, and private-payload-limit state.
    context: ContextCell<'context, 'transaction, 'budget, R, Q>,
    /// Root-inclusive depth of the value currently being serialized.
    depth: usize,
}

impl<'context, 'transaction, 'budget, R, Q> JsonValueSerializer<'context, 'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a serializer for a node at its root-inclusive depth.
    ///
    /// Callers pass depth one for the root and increment it for each child;
    /// all descendants share `context` so configuration and failures propagate.
    pub(in crate::value::json_value_encoder) fn new(
        context: ContextCell<'context, 'transaction, 'budget, R, Q>,
        depth: usize,
    ) -> Self {
        Self { context, depth }
    }

    /// Records an error as the sticky first failure and returns its Serde form.
    fn fail(&self, error: JsonSerializationError) -> JsonSerializationError {
        self.context.borrow_mut().record_serialization_error(error)
    }

    /// Charges a measurement to the staged transaction and retains rejection.
    fn admit(&self, measurement: JsonMeasurement) -> Result<(), JsonSerializationError> {
        let result = { self.context.borrow_mut().admit(measurement) };
        result.map_err(|error| self.fail(error))
    }

    /// Enters this node as a container at its current depth.
    fn enter(&self, kind: JsonContainerKind) -> Result<(), JsonSerializationError> {
        let result = { self.context.borrow_mut().enter_container(kind, self.depth) };
        result.map_err(|error| self.fail(error))
    }

    /// Records collection length or depth arithmetic overflow for `kind`.
    fn collection_error(&self, kind: JsonCollectionKind) -> JsonSerializationError {
        self.fail(JsonSerializationError::new(
            JsonSerializationErrorKind::CollectionLengthOverflow { kind },
        ))
    }

    /// Accounts for a number using its compact JSON lexical byte length.
    fn number(&self, number: Number) -> Result<SafeJsonValue, JsonSerializationError> {
        let bytes = crate::value::internal::json_number_lexeme_length(&number);
        let depth = self.depth;
        self.admit(JsonMeasurement::Number { depth, bytes })?;
        Ok(SafeJsonValue::new(Value::Number(number)))
    }

    /// Accounts for a string using its UTF-8 byte length.
    fn string(&self, text: String) -> Result<SafeJsonValue, JsonSerializationError> {
        let depth = self.depth;
        self.admit(JsonMeasurement::String {
            depth,
            bytes: text.len(),
        })?;
        Ok(SafeJsonValue::new(Value::String(text)))
    }

    /// Checks a prospective container item count without charging it.
    fn container_count(&self, kind: JsonContainerKind, count: usize) -> Result<(), JsonSerializationError> {
        let result = { self.context.borrow_mut().check_container_count(kind, count) };
        result.map_err(|error| self.fail(error))
    }
}

/// Returns a bounded initial allocation hint for a known collection length.
///
/// Unknown lengths produce zero. Known lengths are capped at 1024; this is
/// only a preallocation hint and does not limit accepted collection size.
fn preallocated_capacity(len: Option<usize>) -> usize {
    len.unwrap_or(0).min(1_024)
}

impl<'context, 'transaction, 'budget, R, Q> Serializer for JsonValueSerializer<'context, 'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Completed projected JSON value.
    type Ok = SafeJsonValue;
    /// Structured error used to interrupt Serde serialization.
    type Error = JsonSerializationError;
    /// Sequence, tuple, and tuple struct compound state.
    type SerializeSeq = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;
    /// Tuple compound state.
    type SerializeTuple = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;
    /// Tuple struct compound state.
    type SerializeTupleStruct = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;
    /// Tuple variant compound state.
    type SerializeTupleVariant = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;
    /// Map compound state.
    type SerializeMap = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;
    /// Struct compound state, including recognized private protocols.
    type SerializeStruct = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;
    /// Struct variant compound state.
    type SerializeStructVariant = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;

    /// Serializes a boolean and charges one boolean measurement at this depth.
    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::Boolean { depth: self.depth })?;
        Ok(SafeJsonValue::new(Value::Bool(value)))
    }

    /// Serializes a signed 8-bit integer through the signed 64-bit path.
    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(value.into())
    }
    /// Serializes a signed 16-bit integer through the signed 64-bit path.
    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(value.into())
    }
    /// Serializes a signed 32-bit integer through the signed 64-bit path.
    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(value.into())
    }

    /// Serializes a signed 64-bit integer using its compact JSON number form.
    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        self.number(value.into())
    }

    /// Serializes a signed 128-bit integer if it fits JSON's supported range.
    ///
    /// Returns an integer-out-of-range error when it fits neither `i64` nor
    /// `u64`.
    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        if let Ok(value) = i64::try_from(value) {
            return self.serialize_i64(value);
        }
        if let Ok(value) = u64::try_from(value) {
            return self.serialize_u64(value);
        }
        Err(self.fail(JsonSerializationError::new(
            JsonSerializationErrorKind::IntegerOutOfRange {
                signedness: JsonIntegerSignedness::Signed,
            },
        )))
    }

    /// Serializes an unsigned 8-bit integer through the unsigned 64-bit path.
    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(value.into())
    }
    /// Serializes an unsigned 16-bit integer through the unsigned 64-bit path.
    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(value.into())
    }
    /// Serializes an unsigned 32-bit integer through the unsigned 64-bit path.
    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(value.into())
    }

    /// Serializes an unsigned 64-bit integer using its compact JSON number
    /// form.
    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        self.number(value.into())
    }

    /// Serializes an unsigned 128-bit integer when it fits in `u64`.
    ///
    /// Returns an integer-out-of-range error otherwise.
    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        match u64::try_from(value) {
            Ok(value) => self.serialize_u64(value),
            Err(_) => Err(self.fail(JsonSerializationError::new(
                JsonSerializationErrorKind::IntegerOutOfRange {
                    signedness: JsonIntegerSignedness::Unsigned,
                },
            ))),
        }
    }

    /// Serializes a finite 32-bit float through serde_json's number parser.
    ///
    /// Non-finite values and invalid JSON number representations are rejected.
    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        if !value.is_finite() {
            return Err(self.fail(JsonSerializationError::new(JsonSerializationErrorKind::NonFiniteFloat)));
        }
        match to_string(&value).and_then(|text| serde_json::from_str::<Number>(&text)) {
            Ok(number) => self.number(number),
            Err(_) => Err(self.fail(JsonSerializationError::new(
                JsonSerializationErrorKind::InvalidNumberRepresentation,
            ))),
        }
    }

    /// Serializes a finite 64-bit float as a JSON number.
    ///
    /// Non-finite values are rejected.
    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        match Number::from_f64(value) {
            Some(number) => self.number(number),
            None => Err(self.fail(JsonSerializationError::new(JsonSerializationErrorKind::NonFiniteFloat))),
        }
    }

    /// Serializes a character as a string, charging its UTF-8 byte length.
    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::String {
            depth: self.depth,
            bytes: value.len_utf8(),
        })?;
        Ok(SafeJsonValue::new(Value::String(value.to_string())))
    }
    /// Serializes a string slice and charges its UTF-8 byte length.
    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::String {
            depth: self.depth,
            bytes: value.len(),
        })?;
        Ok(SafeJsonValue::new(Value::String(value.to_owned())))
    }

    /// Serializes bytes as a JSON array of numbers, accounting for each
    /// element.
    fn serialize_bytes(self, bytes: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.enter(JsonContainerKind::Sequence)?;
        let mut values = Vec::with_capacity(preallocated_capacity(Some(bytes.len())));
        for (index, byte) in bytes.iter().enumerate() {
            let count = index
                .checked_add(1)
                .ok_or_else(|| self.collection_error(JsonCollectionKind::Array))?;
            self.container_count(JsonContainerKind::Sequence, count)?;
            let depth = self
                .depth
                .checked_add(1)
                .ok_or_else(|| self.collection_error(JsonCollectionKind::Array))?;
            let number: Number = (*byte).into();
            let number_bytes = crate::value::internal::json_number_lexeme_length(&number);
            self.admit(JsonMeasurement::Number {
                depth,
                bytes: number_bytes,
            })?;
            values.push(SafeJsonValue::new(Value::Number(number)));
        }
        let mut builder = SafeValueBuilder::array();
        for value in values {
            builder.push(value);
        }
        Ok(SafeJsonValue::new(builder.finish()))
    }

    /// Serializes `None` as JSON null.
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }
    /// Serializes the contained value without adding a wrapper.
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    /// Serializes unit as JSON null.
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
        Ok(SafeJsonValue::new(Value::Null))
    }

    /// Serializes a unit struct as JSON null.
    fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    /// Serializes a unit enum variant as its variant name string.
    fn serialize_unit_variant(self, _: &'static str, _: u32, variant: &'static str) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::String {
            depth: self.depth,
            bytes: variant.len(),
        })?;
        Ok(SafeJsonValue::new(Value::String(variant.to_owned())))
    }

    /// Serializes a newtype struct by delegating directly to its value.
    fn serialize_newtype_struct<T>(self, _: &'static str, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    /// Serializes a newtype enum variant as a one-entry JSON object.
    fn serialize_newtype_variant<T>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.enter(JsonContainerKind::Map)?;
        self.container_count(JsonContainerKind::Map, 1)?;
        self.admit(JsonMeasurement::Key { bytes: variant.len() })?;
        let child_depth = self
            .depth
            .checked_add(1)
            .ok_or_else(|| self.collection_error(JsonCollectionKind::Object))?;
        let child = value.serialize(Self::new(self.context, child_depth))?;
        let mut object = SafeValueBuilder::object();
        object.insert(variant.to_owned(), child);
        Ok(SafeJsonValue::new(object.finish()))
    }

    /// Starts a sequence compound; its reported length is an allocation hint.
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.enter(JsonContainerKind::Sequence)?;
        Ok(JsonValueCompound::sequence(
            self.context,
            self.depth,
            JsonContainerKind::Sequence,
            preallocated_capacity(len),
        ))
    }

    /// Starts a tuple using its fixed length as an allocation hint.
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }
    /// Starts a tuple struct using its fixed length as an allocation hint.
    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    /// Starts a tuple enum variant represented as a one-entry object.
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.enter(JsonContainerKind::Map)?;
        self.container_count(JsonContainerKind::Map, 1)?;
        self.admit(JsonMeasurement::Key { bytes: variant.len() })?;
        let depth = self
            .depth
            .checked_add(1)
            .ok_or_else(|| self.collection_error(JsonCollectionKind::Object))?;
        let result = {
            self.context
                .borrow_mut()
                .enter_container(JsonContainerKind::Sequence, depth)
        };
        result.map_err(|error| self.fail(error))?;
        Ok(JsonValueCompound::tuple_variant(
            self.context,
            variant,
            self.depth,
            preallocated_capacity(Some(len)),
        ))
    }

    /// Starts an ordinary map compound.
    ///
    /// Map events are treated as user map entries and do not identify
    /// serde_json's private `Number` protocol; reserved private struct names
    /// are recognized by `serialize_struct` instead.
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.enter(JsonContainerKind::Map)?;
        Ok(JsonValueCompound::map(
            self.context,
            self.depth,
            preallocated_capacity(len),
        ))
    }

    /// Starts a struct compound or a recognized private serde_json protocol.
    ///
    /// Private raw-value and number payloads are selected by their reserved
    /// struct names. Other structs use ordinary map accounting at this node's
    /// root-inclusive depth.
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        if let Some(kind) = JsonPrivateProtocolKind::from_struct_name(name) {
            let state = JsonPrivateProtocolState::new(kind, len).map_err(|reason| {
                self.fail(JsonSerializationError::new(
                    JsonSerializationErrorKind::InvalidSerializerState { reason },
                ))
            })?;
            return Ok(JsonValueCompound::private(self.context, self.depth, kind, state));
        }
        self.enter(JsonContainerKind::Map)?;
        Ok(JsonValueCompound::map(
            self.context,
            self.depth,
            preallocated_capacity(Some(len)),
        ))
    }

    /// Starts a struct enum variant represented as a one-entry object.
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.enter(JsonContainerKind::Map)?;
        self.container_count(JsonContainerKind::Map, 1)?;
        self.admit(JsonMeasurement::Key { bytes: variant.len() })?;
        let depth = self
            .depth
            .checked_add(1)
            .ok_or_else(|| self.collection_error(JsonCollectionKind::Object))?;
        let result = { self.context.borrow_mut().enter_container(JsonContainerKind::Map, depth) };
        result.map_err(|error| self.fail(error))?;
        Ok(JsonValueCompound::struct_variant(
            self.context,
            variant,
            self.depth,
            preallocated_capacity(Some(len)),
        ))
    }

    /// Formats `value` into a string, charging bytes emitted by its display
    /// implementation.
    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display + ?Sized,
    {
        let mut collector =
            JsonValueDisplayCollector::new(self.context, JsonValueDisplayKind::String { depth: self.depth });
        if write!(&mut collector, "{value}").is_err() {
            return Err(self.fail(JsonSerializationError::new(
                JsonSerializationErrorKind::DisplayFormattingFailed,
            )));
        }
        let text = collector.into_string();
        self.string(text)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use qubit_budget::json::JsonResource;
    use qubit_budget::json::JsonValueBudget;
    use qubit_budget::json::JsonValueLimits;
    use serde::Serializer;

    use super::JsonValueEncodeContext;
    use super::JsonValueSerializer;
    use crate::encode::JsonCollectionKind;
    use crate::encode::JsonSerializationErrorKind;

    #[test]
    fn test_serializer_depth_overflow_preserves_collection_kind() {
        for path in ["bytes", "newtype", "tuple", "struct"] {
            let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
            let mut transaction = budget.transaction();
            let context = RefCell::new(JsonValueEncodeContext::new(&mut transaction, None));
            let serializer = JsonValueSerializer::new(&context, usize::MAX);
            let result = match path {
                "bytes" => serializer.serialize_bytes(&[1]).map(|_| ()),
                "newtype" => serializer
                    .serialize_newtype_variant("Enum", 0, "New", &true)
                    .map(|_| ()),
                "tuple" => serializer.serialize_tuple_variant("Enum", 0, "Tuple", 1).map(|_| ()),
                _ => serializer.serialize_struct_variant("Enum", 0, "Struct", 1).map(|_| ()),
            };
            let error = result.expect_err("child depth must not wrap");
            assert_eq!(
                error.kind(),
                JsonSerializationErrorKind::CollectionLengthOverflow {
                    kind: if path == "bytes" {
                        JsonCollectionKind::Array
                    } else {
                        JsonCollectionKind::Object
                    },
                }
            );
            assert_eq!(context.borrow().ser_error(), error);
        }
    }
}
