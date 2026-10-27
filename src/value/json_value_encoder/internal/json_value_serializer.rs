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
use super::JsonValueEncodeContext;
use crate::encode::JsonCollectionKind;
use crate::encode::JsonIntegerSignedness;
use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;
use crate::internal::JsonPrivateProtocolKind;
use crate::internal::JsonPrivateProtocolState;
use crate::value::SafeJsonValue;
use crate::value::safe_json_value::SafeValueBuilder;

type ContextCell<'context, 'transaction, 'budget, R, Q> =
    &'context RefCell<JsonValueEncodeContext<'transaction, 'budget, R, Q>>;

/// Serializer facade for one node at its root-inclusive depth.
#[derive(Clone, Copy)]
pub(in crate::value::json_value_encoder) struct JsonValueSerializer<'context, 'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    context: ContextCell<'context, 'transaction, 'budget, R, Q>,
    depth: usize,
}

impl<'context, 'transaction, 'budget, R, Q> JsonValueSerializer<'context, 'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    pub(in crate::value::json_value_encoder) fn new(
        context: ContextCell<'context, 'transaction, 'budget, R, Q>,
        depth: usize,
    ) -> Self {
        Self { context, depth }
    }

    fn fail(&self, error: JsonSerializationError) -> JsonSerializationError {
        self.context.borrow_mut().record_serialization_error(error)
    }

    fn admit(&self, measurement: JsonMeasurement) -> Result<(), JsonSerializationError> {
        let result = { self.context.borrow_mut().admit(measurement) };
        result.map_err(|error| self.fail(error))
    }

    fn enter(&self, kind: JsonContainerKind) -> Result<(), JsonSerializationError> {
        let result = { self.context.borrow_mut().enter_container(kind, self.depth) };
        result.map_err(|error| self.fail(error))
    }

    fn collection_error(&self, kind: JsonCollectionKind) -> JsonSerializationError {
        self.fail(JsonSerializationError::new(
            JsonSerializationErrorKind::CollectionLengthOverflow { kind },
        ))
    }

    fn number(&self, number: Number) -> Result<SafeJsonValue, JsonSerializationError> {
        let bytes = crate::value::internal::json_number_lexeme_length(&number);
        let depth = self.depth;
        self.admit(JsonMeasurement::Number { depth, bytes })?;
        Ok(SafeJsonValue::new(Value::Number(number)))
    }

    fn string(&self, text: String) -> Result<SafeJsonValue, JsonSerializationError> {
        let depth = self.depth;
        self.admit(JsonMeasurement::String {
            depth,
            bytes: text.len(),
        })?;
        Ok(SafeJsonValue::new(Value::String(text)))
    }

    fn container_count(&self, kind: JsonContainerKind, count: usize) -> Result<(), JsonSerializationError> {
        let result = { self.context.borrow_mut().check_container_count(kind, count) };
        result.map_err(|error| self.fail(error))
    }
}

fn preallocated_capacity(len: Option<usize>) -> usize {
    len.unwrap_or(0).min(1_024)
}

impl<'context, 'transaction, 'budget, R, Q> Serializer for JsonValueSerializer<'context, 'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = SafeJsonValue;
    type Error = JsonSerializationError;
    type SerializeSeq = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;
    type SerializeTuple = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;
    type SerializeTupleStruct = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;
    type SerializeTupleVariant = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;
    type SerializeMap = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;
    type SerializeStruct = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;
    type SerializeStructVariant = JsonValueCompound<'context, 'transaction, 'budget, R, Q>;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::Boolean { depth: self.depth })?;
        Ok(SafeJsonValue::new(Value::Bool(value)))
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(value.into())
    }
    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(value.into())
    }
    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(value.into())
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        self.number(value.into())
    }

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

    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(value.into())
    }
    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(value.into())
    }
    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(value.into())
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        self.number(value.into())
    }

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

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        match Number::from_f64(value) {
            Some(number) => self.number(number),
            None => Err(self.fail(JsonSerializationError::new(JsonSerializationErrorKind::NonFiniteFloat))),
        }
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::String {
            depth: self.depth,
            bytes: value.len_utf8(),
        })?;
        Ok(SafeJsonValue::new(Value::String(value.to_string())))
    }
    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::String {
            depth: self.depth,
            bytes: value.len(),
        })?;
        Ok(SafeJsonValue::new(Value::String(value.to_owned())))
    }

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

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
        Ok(SafeJsonValue::new(Value::Null))
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(self, _: &'static str, _: u32, variant: &'static str) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::String {
            depth: self.depth,
            bytes: variant.len(),
        })?;
        Ok(SafeJsonValue::new(Value::String(variant.to_owned())))
    }

    fn serialize_newtype_struct<T>(self, _: &'static str, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

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

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.enter(JsonContainerKind::Sequence)?;
        Ok(JsonValueCompound::sequence(
            self.context,
            self.depth,
            JsonContainerKind::Sequence,
            preallocated_capacity(len),
        ))
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

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

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.enter(JsonContainerKind::Map)?;
        Ok(JsonValueCompound::map(
            self.context,
            self.depth,
            preallocated_capacity(len),
        ))
    }

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

    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display + ?Sized,
    {
        let mut text = String::new();
        write!(&mut text, "{value}").map_err(|_| {
            self.fail(JsonSerializationError::new(
                JsonSerializationErrorKind::DisplayFormattingFailed,
            ))
        })?;
        self.string(text)
    }
}
