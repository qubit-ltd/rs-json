// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Transaction-aware Serde compound serializers.

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::BTreeSet;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonContainerKind;
use qubit_budget::json::JsonMeasurement;
use serde::Serialize;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;
use serde::ser::SerializeStructVariant;
use serde::ser::SerializeTuple;
use serde::ser::SerializeTupleStruct;
use serde::ser::SerializeTupleVariant;
use serde_json::Value;

use super::JsonValueEncodeContext;
use super::JsonValueMapKeySerializer;
use super::JsonValueSerializer;
use super::private_text_serializer::PrivateTextSerializer;
use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;
use crate::encode::JsonSerializerStateError;
use crate::internal::JsonPrivateProtocolKind;
use crate::internal::JsonPrivateProtocolState;
use crate::value::SafeJsonValue;
use crate::value::internal::SafeValueBuilder;

type ContextCell<'context, 'transaction, 'budget, R, Q> =
    &'context RefCell<JsonValueEncodeContext<'transaction, 'budget, R, Q>>;

enum CompoundState {
    Sequence {
        values: Vec<SafeJsonValue>,
        depth: usize,
        count: usize,
        kind: JsonContainerKind,
    },
    Map {
        values: Vec<(String, SafeJsonValue)>,
        keys: BTreeSet<String>,
        next_key: Option<String>,
        depth: usize,
        count: usize,
    },
    TupleVariant {
        variant: String,
        values: Vec<SafeJsonValue>,
        depth: usize,
        count: usize,
    },
    StructVariant {
        variant: String,
        values: Vec<(String, SafeJsonValue)>,
        keys: BTreeSet<String>,
        depth: usize,
        count: usize,
    },
    RawValue {
        text: Option<String>,
        state: JsonPrivateProtocolState,
        depth: usize,
    },
    Number {
        text: Option<String>,
        state: JsonPrivateProtocolState,
        depth: usize,
    },
}

/// Accumulates one Serde compound with guarded children and sticky failures.
///
/// Children remain wrapped in `SafeJsonValue` until the compound ends
/// successfully; a failed child or measurement prevents the partial tree from
/// being returned. Private raw-value and number payloads use dedicated protocol
/// states. A generic map event sequence is treated as a map and cannot reveal
/// that it originated from serde_json's private `Number` representation.
pub(in crate::value::json_value_encoder) struct JsonValueCompound<'context, 'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    context: ContextCell<'context, 'transaction, 'budget, R, Q>,
    state: CompoundState,
    failure: Cell<Option<JsonSerializationError>>,
}

impl<'context, 'transaction, 'budget, R, Q> JsonValueCompound<'context, 'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    pub(in crate::value::json_value_encoder) fn sequence(
        context: ContextCell<'context, 'transaction, 'budget, R, Q>,
        depth: usize,
        kind: JsonContainerKind,
        capacity: usize,
    ) -> Self {
        Self {
            context,
            state: CompoundState::Sequence {
                values: Vec::with_capacity(capacity),
                depth,
                count: 0,
                kind,
            },
            failure: Cell::new(None),
        }
    }

    pub(in crate::value::json_value_encoder) fn map(
        context: ContextCell<'context, 'transaction, 'budget, R, Q>,
        depth: usize,
        capacity: usize,
    ) -> Self {
        Self {
            context,
            state: CompoundState::Map {
                values: Vec::with_capacity(capacity),
                keys: BTreeSet::new(),
                next_key: None,
                depth,
                count: 0,
            },
            failure: Cell::new(None),
        }
    }

    pub(in crate::value::json_value_encoder) fn tuple_variant(
        context: ContextCell<'context, 'transaction, 'budget, R, Q>,
        variant: &str,
        depth: usize,
        capacity: usize,
    ) -> Self {
        Self {
            context,
            state: CompoundState::TupleVariant {
                variant: variant.to_owned(),
                values: Vec::with_capacity(capacity),
                depth,
                count: 0,
            },
            failure: Cell::new(None),
        }
    }

    pub(in crate::value::json_value_encoder) fn struct_variant(
        context: ContextCell<'context, 'transaction, 'budget, R, Q>,
        variant: &str,
        depth: usize,
        capacity: usize,
    ) -> Self {
        Self {
            context,
            state: CompoundState::StructVariant {
                variant: variant.to_owned(),
                values: Vec::with_capacity(capacity),
                keys: BTreeSet::new(),
                depth,
                count: 0,
            },
            failure: Cell::new(None),
        }
    }

    pub(in crate::value::json_value_encoder) fn private(
        context: ContextCell<'context, 'transaction, 'budget, R, Q>,
        depth: usize,
        kind: JsonPrivateProtocolKind,
        state: JsonPrivateProtocolState,
    ) -> Self {
        let state = match kind {
            JsonPrivateProtocolKind::RawValue => CompoundState::RawValue {
                text: None,
                state,
                depth,
            },
            JsonPrivateProtocolKind::Number => CompoundState::Number {
                text: None,
                state,
                depth,
            },
        };
        Self {
            context,
            state,
            failure: Cell::new(None),
        }
    }

    /// Retains this compound's first serialization failure and records it in
    /// the shared context.
    fn fail(&self, error: JsonSerializationError) -> JsonSerializationError {
        if self.failure.get().is_none() {
            self.failure.set(Some(error));
        }
        let _ = self.context.borrow_mut().record_serialization_error(error);
        self.failure.get().unwrap_or(error)
    }

    fn keep_end_error<T>(
        context: ContextCell<'_, '_, '_, R, Q>,
        result: Result<T, JsonSerializationError>,
    ) -> Result<T, JsonSerializationError> {
        result.map_err(|error| context.borrow_mut().record_serialization_error(error))
    }

    /// Stops this compound when either its local or shared context has failed.
    fn check_failure(&self) -> Result<(), JsonSerializationError> {
        if let Some(error) = self.failure.get() {
            return Err(error);
        }
        let failed = self.context.borrow().has_failure();
        if failed {
            let error = self.context.borrow().ser_error();
            return Err(error);
        }
        Ok(())
    }

    fn admit(&mut self, measurement: JsonMeasurement) -> Result<(), JsonSerializationError> {
        let result = { self.context.borrow_mut().admit(measurement) };
        result.map_err(|error| self.fail(error))
    }

    fn serialize_child<T>(&mut self, value: &T, depth: usize) -> Result<SafeJsonValue, JsonSerializationError>
    where
        T: Serialize + ?Sized,
    {
        match value.serialize(JsonValueSerializer::new(self.context, depth)) {
            Ok(value) => {
                self.check_failure()?;
                Ok(value)
            }
            Err(error) => Err(self.fail(error)),
        }
    }

    fn check_count(&mut self, kind: JsonContainerKind, count: usize) -> Result<(), JsonSerializationError> {
        let result = { self.context.borrow_mut().check_container_count(kind, count) };
        result.map_err(|error| self.fail(error))
    }

    /// Validates and stores one field of the recognized private protocol.
    ///
    /// Private text is checked against the configured per-payload byte limit
    /// before the payload string is appended to its buffer. Protocol shape or
    /// payload errors become sticky.
    fn set_private_field<T>(
        &mut self,
        kind: JsonPrivateProtocolKind,
        key: &'static str,
        value: &T,
    ) -> Result<(), JsonSerializationError>
    where
        T: Serialize + ?Sized,
    {
        let context = self.context;
        let mut check_text_bytes = |length| context.borrow_mut().check_private_text_bytes(length);
        let result = match &mut self.state {
            CompoundState::RawValue { text, state, .. } | CompoundState::Number { text, state, .. } => {
                set_private_payload(text, state, kind, key, value, &mut check_text_bytes)
            }
            _ => Err(invalid_state(JsonSerializerStateError::UnexpectedCompound)),
        };
        result.map_err(|error| self.fail(error))
    }

    fn finish_sequence(values: Vec<SafeJsonValue>) -> SafeJsonValue {
        let mut builder = SafeValueBuilder::array();
        for value in values {
            builder.push(value);
        }
        SafeJsonValue::new(builder.finish())
    }

    fn finish_object(values: Vec<(String, SafeJsonValue)>) -> SafeJsonValue {
        let mut object = SafeValueBuilder::object();
        for (key, value) in values {
            object.insert(key, value);
        }
        SafeJsonValue::new(object.finish())
    }

    fn finish_private(self) -> Result<SafeJsonValue, JsonSerializationError> {
        let context = self.context;
        self.finish_private_inner()
            .map_err(|error| context.borrow_mut().record_serialization_error(error))
    }

    /// Validates the complete private protocol before parsing/admitting its
    /// payload.
    fn finish_private_inner(self) -> Result<SafeJsonValue, JsonSerializationError> {
        match &self.state {
            CompoundState::RawValue {
                text: Some(text),
                state,
                depth,
            } => {
                if state.finish().is_err() {
                    return Err(private_invalid_state(JsonPrivateProtocolKind::RawValue));
                }
                decode_raw_value(text, self.context, *depth)
            }
            CompoundState::RawValue { state, .. } => {
                let _ = state.finish();
                Err(private_invalid_state(JsonPrivateProtocolKind::RawValue))
            }
            CompoundState::Number {
                text: Some(text),
                state,
                depth,
            } => {
                if state.finish().is_err() {
                    return Err(private_invalid_state(JsonPrivateProtocolKind::Number));
                }
                match crate::internal::StrictJsonNumber::parse(text) {
                    Ok(number) => {
                        let bytes = crate::value::internal::json_number_lexeme_length(&number);
                        self.context
                            .borrow_mut()
                            .admit(JsonMeasurement::Number { depth: *depth, bytes })?;
                        Ok(SafeJsonValue::new(Value::Number(number)))
                    }
                    Err(error) => Err(self.context.borrow_mut().record_serialization_error(error)),
                }
            }
            CompoundState::Number { state, .. } => {
                let _ = state.finish();
                Err(private_invalid_state(JsonPrivateProtocolKind::Number))
            }
            _ => Err(invalid_state(JsonSerializerStateError::UnexpectedCompound)),
        }
    }
}

fn set_private_payload<T>(
    text: &mut Option<String>,
    state: &mut JsonPrivateProtocolState,
    kind: JsonPrivateProtocolKind,
    key: &'static str,
    value: &T,
    check_text_bytes: &mut dyn FnMut(usize) -> Result<(), JsonSerializationError>,
) -> Result<(), JsonSerializationError>
where
    T: Serialize + ?Sized,
{
    if state.begin(key).is_err() {
        return Err(private_invalid_state(kind));
    }
    let value = match value.serialize(PrivateTextSerializer { kind, check_text_bytes }) {
        Ok(Value::String(value)) => value,
        Ok(_) => {
            state.fail();
            return Err(private_invalid_state(kind));
        }
        Err(error) => {
            state.fail();
            return Err(error);
        }
    };
    *text = Some(value);
    if state.complete().is_err() {
        return Err(private_invalid_state(kind));
    }
    Ok(())
}

impl<R, Q> SerializeSeq for JsonValueCompound<'_, '_, '_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = SafeJsonValue;
    type Error = JsonSerializationError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.check_failure()?;
        let (depth, count, kind) = match &self.state {
            CompoundState::Sequence { depth, count, kind, .. } => (*depth, *count, *kind),
            _ => return Err(self.fail(invalid_state(JsonSerializerStateError::UnexpectedCompound))),
        };
        let next_count = count.checked_add(1).ok_or_else(|| {
            self.fail(JsonSerializationError::new(
                JsonSerializationErrorKind::CollectionLengthOverflow {
                    kind: crate::encode::JsonCollectionKind::Array,
                },
            ))
        })?;
        self.check_count(kind, next_count)?;
        let child_depth = depth
            .checked_add(1)
            .ok_or_else(|| self.fail(invalid_state(JsonSerializerStateError::UnexpectedCompound)))?;
        let child = self.serialize_child(value, child_depth)?;
        let CompoundState::Sequence { values, count, .. } = &mut self.state else {
            unreachable!()
        };
        values.push(child);
        *count = next_count;
        Ok(())
    }

    /// Completes the compound only when no earlier failure or incomplete
    /// protocol remains.
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.check_failure()?;
        let context = self.context;
        let result = match self.state {
            CompoundState::Sequence { values, .. } => Ok(Self::finish_sequence(values)),
            _ => Err(invalid_state(JsonSerializerStateError::UnexpectedCompound)),
        };
        Self::keep_end_error(context, result)
    }
}

impl<R, Q> SerializeTuple for JsonValueCompound<'_, '_, '_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = SafeJsonValue;
    type Error = JsonSerializationError;
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl<R, Q> SerializeTupleStruct for JsonValueCompound<'_, '_, '_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = SafeJsonValue;
    type Error = JsonSerializationError;
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl<R, Q> SerializeTupleVariant for JsonValueCompound<'_, '_, '_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = SafeJsonValue;
    type Error = JsonSerializationError;
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.check_failure()?;
        let (depth, count) = match &self.state {
            CompoundState::TupleVariant { depth, count, .. } => (*depth, *count),
            _ => return Err(self.fail(invalid_state(JsonSerializerStateError::UnexpectedCompound))),
        };
        let next = count.checked_add(1).ok_or_else(|| {
            self.fail(JsonSerializationError::new(
                JsonSerializationErrorKind::CollectionLengthOverflow {
                    kind: crate::encode::JsonCollectionKind::Array,
                },
            ))
        })?;
        self.check_count(JsonContainerKind::Sequence, next)?;
        let child_depth = depth
            .checked_add(2)
            .ok_or_else(|| self.fail(invalid_state(JsonSerializerStateError::UnexpectedCompound)))?;
        let child = self.serialize_child(value, child_depth)?;
        let CompoundState::TupleVariant { values, count, .. } = &mut self.state else {
            unreachable!()
        };
        values.push(child);
        *count = next;
        Ok(())
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.check_failure()?;
        let context = self.context;
        let result = match self.state {
            CompoundState::TupleVariant { variant, values, .. } => {
                let inner = Self::finish_sequence(values);
                let mut object = SafeValueBuilder::object();
                object.insert(variant, inner);
                Ok(SafeJsonValue::new(object.finish()))
            }
            _ => Err(invalid_state(JsonSerializerStateError::UnexpectedCompound)),
        };
        Self::keep_end_error(context, result)
    }
}

impl<R, Q> SerializeMap for JsonValueCompound<'_, '_, '_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = SafeJsonValue;
    type Error = JsonSerializationError;
    /// Serializes and checks the prospective key before the corresponding value
    /// is admitted.
    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.check_failure()?;
        let (count, pending) = match &self.state {
            CompoundState::Map { count, next_key, .. } => (*count, next_key.is_some()),
            _ => return Err(self.fail(invalid_state(JsonSerializerStateError::UnexpectedCompound))),
        };
        if pending {
            return Err(self.fail(invalid_state(JsonSerializerStateError::MapKeyAlreadyPending)));
        }
        let next = count.checked_add(1).ok_or_else(|| {
            self.fail(JsonSerializationError::new(
                JsonSerializationErrorKind::CollectionLengthOverflow {
                    kind: crate::encode::JsonCollectionKind::Object,
                },
            ))
        })?;
        self.check_count(JsonContainerKind::Map, next)?;
        let key = match key.serialize(JsonValueMapKeySerializer::new(self.context)) {
            Ok(key) => key,
            Err(error) => return Err(self.fail(error)),
        };
        self.admit(JsonMeasurement::Key { bytes: key.len() })?;
        let CompoundState::Map { next_key, count, .. } = &mut self.state else {
            unreachable!()
        };
        *next_key = Some(key);
        *count = next;
        Ok(())
    }
    /// Adds a value only after its pending key and guarded child succeed.
    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.check_failure()?;
        let (key, depth) = match &self.state {
            CompoundState::Map {
                keys, next_key, depth, ..
            } => {
                let Some(key) = next_key.as_ref() else {
                    return Err(self.fail(invalid_state(JsonSerializerStateError::MapValueWithoutKey)));
                };
                if keys.contains(key.as_str()) {
                    return Err(self.fail(JsonSerializationError::new(
                        JsonSerializationErrorKind::DuplicateObjectKey,
                    )));
                }
                (key.clone(), *depth)
            }
            _ => return Err(self.fail(invalid_state(JsonSerializerStateError::UnexpectedCompound))),
        };
        if let CompoundState::Map { keys, .. } = &mut self.state {
            keys.insert(key.clone());
        }
        let child_depth = depth
            .checked_add(1)
            .ok_or_else(|| self.fail(invalid_state(JsonSerializerStateError::UnexpectedCompound)))?;
        if let CompoundState::Map { next_key, .. } = &mut self.state {
            *next_key = None;
        }
        let child = self.serialize_child(value, child_depth)?;
        let CompoundState::Map { values, .. } = &mut self.state else {
            unreachable!()
        };
        values.push((key, child));
        Ok(())
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.check_failure()?;
        let context = self.context;
        let result = match self.state {
            CompoundState::Map {
                values, next_key: None, ..
            } => Ok(Self::finish_object(values)),
            CompoundState::Map { next_key: Some(_), .. } => {
                Err(invalid_state(JsonSerializerStateError::MapEndedWithPendingKey))
            }
            _ => Err(invalid_state(JsonSerializerStateError::UnexpectedCompound)),
        };
        Self::keep_end_error(context, result)
    }
}

impl<R, Q> SerializeStruct for JsonValueCompound<'_, '_, '_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = SafeJsonValue;
    type Error = JsonSerializationError;
    fn skip_field(&mut self, _: &'static str) -> Result<(), Self::Error> {
        match &self.state {
            CompoundState::RawValue { .. } => Err(self.fail(private_invalid_state(JsonPrivateProtocolKind::RawValue))),
            CompoundState::Number { .. } => Err(self.fail(private_invalid_state(JsonPrivateProtocolKind::Number))),
            _ => Ok(()),
        }
    }
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.check_failure()?;
        match &self.state {
            CompoundState::Map { keys, depth, count, .. } => {
                let (depth, count, duplicate) = (*depth, *count, keys.contains(key));
                if duplicate {
                    return Err(self.fail(JsonSerializationError::new(
                        JsonSerializationErrorKind::DuplicateObjectKey,
                    )));
                }
                let next = count.checked_add(1).ok_or_else(|| {
                    self.fail(JsonSerializationError::new(
                        JsonSerializationErrorKind::CollectionLengthOverflow {
                            kind: crate::encode::JsonCollectionKind::Object,
                        },
                    ))
                })?;
                self.check_count(JsonContainerKind::Map, next)?;
                self.admit(JsonMeasurement::Key { bytes: key.len() })?;
                if let CompoundState::Map { keys, .. } = &mut self.state {
                    keys.insert(key.to_owned());
                }
                let child_depth = depth
                    .checked_add(1)
                    .ok_or_else(|| self.fail(invalid_state(JsonSerializerStateError::UnexpectedCompound)))?;
                let child = self.serialize_child(value, child_depth)?;
                let CompoundState::Map { values, count, .. } = &mut self.state else {
                    unreachable!()
                };
                values.push((key.to_owned(), child));
                *count = next;
                Ok(())
            }
            CompoundState::RawValue { .. } => self.set_private_field(JsonPrivateProtocolKind::RawValue, key, value),
            CompoundState::Number { .. } => self.set_private_field(JsonPrivateProtocolKind::Number, key, value),
            _ => Err(self.fail(invalid_state(JsonSerializerStateError::UnexpectedCompound))),
        }
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.check_failure()?;
        if matches!(
            &self.state,
            CompoundState::RawValue { .. } | CompoundState::Number { .. }
        ) {
            return self.finish_private();
        }
        let context = self.context;
        let result = match self.state {
            CompoundState::Map {
                values, next_key: None, ..
            } => Ok(Self::finish_object(values)),
            CompoundState::Map { next_key: Some(_), .. } => {
                Err(invalid_state(JsonSerializerStateError::MapEndedWithPendingKey))
            }
            _ => Err(invalid_state(JsonSerializerStateError::UnexpectedCompound)),
        };
        Self::keep_end_error(context, result)
    }
}

impl<R, Q> SerializeStructVariant for JsonValueCompound<'_, '_, '_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = SafeJsonValue;
    type Error = JsonSerializationError;
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.check_failure()?;
        let (depth, count, duplicate) = match &self.state {
            CompoundState::StructVariant { keys, depth, count, .. } => (*depth, *count, keys.contains(key)),
            _ => return Err(self.fail(invalid_state(JsonSerializerStateError::UnexpectedCompound))),
        };
        if duplicate {
            return Err(self.fail(JsonSerializationError::new(
                JsonSerializationErrorKind::DuplicateObjectKey,
            )));
        }
        let next = count.checked_add(1).ok_or_else(|| {
            self.fail(JsonSerializationError::new(
                JsonSerializationErrorKind::CollectionLengthOverflow {
                    kind: crate::encode::JsonCollectionKind::Object,
                },
            ))
        })?;
        self.check_count(JsonContainerKind::Map, next)?;
        self.admit(JsonMeasurement::Key { bytes: key.len() })?;
        if let CompoundState::StructVariant { keys, .. } = &mut self.state {
            keys.insert(key.to_owned());
        }
        let child_depth = depth
            .checked_add(2)
            .ok_or_else(|| self.fail(invalid_state(JsonSerializerStateError::UnexpectedCompound)))?;
        let child = self.serialize_child(value, child_depth)?;
        let CompoundState::StructVariant { values, count, .. } = &mut self.state else {
            unreachable!()
        };
        values.push((key.to_owned(), child));
        *count = next;
        Ok(())
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.check_failure()?;
        let context = self.context;
        let result = match self.state {
            CompoundState::StructVariant { variant, values, .. } => {
                let inner = Self::finish_object(values);
                let mut outer = SafeValueBuilder::object();
                outer.insert(variant, inner);
                Ok(SafeJsonValue::new(outer.finish()))
            }
            _ => Err(invalid_state(JsonSerializerStateError::UnexpectedCompound)),
        };
        Self::keep_end_error(context, result)
    }
}

fn invalid_state(reason: JsonSerializerStateError) -> JsonSerializationError {
    JsonSerializationError::new(JsonSerializationErrorKind::InvalidSerializerState { reason })
}

pub(super) fn private_invalid_state(kind: JsonPrivateProtocolKind) -> JsonSerializationError {
    invalid_state(kind.invalid_state())
}

struct RawValueConsumer<'builder> {
    builder: &'builder mut crate::value::internal::LexicalJsonValueBuilder,
}

impl<'input> crate::lexical::JsonLexicalConsumer<'input> for RawValueConsumer<'_> {
    type Error = crate::value::internal::JsonValueMaterializationError;
    fn before_value(&mut self, _: usize, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    fn before_key(&mut self, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    fn before_number(&mut self, _: &'input str, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    fn event(&mut self, event: crate::lexical::JsonLexicalEvent<'input>) -> Result<(), Self::Error> {
        self.builder.try_accept(event)
    }
}

fn decode_raw_value<R, Q>(
    text: &str,
    context: ContextCell<'_, '_, '_, R, Q>,
    depth: usize,
) -> Result<SafeJsonValue, JsonSerializationError>
where
    R: Clone,
    Q: ResourceQuantity,
{
    let mut builder = crate::value::internal::LexicalJsonValueBuilder::new(crate::lexical::JsonNumberModel::Integer64);
    let has_limits = context.borrow().has_limits();
    let scan = {
        let mut ctx = context.borrow_mut();
        let transaction = ctx.transaction();
        crate::lexical::JsonLexicalScanner::at_depth(transaction, depth, has_limits)
            .with_number_model(crate::lexical::JsonNumberModel::Integer64)
            .scan_with_consumer(text.as_bytes(), &mut RawValueConsumer { builder: &mut builder })
    };
    match scan {
        Ok(()) => match builder.finish_guarded() {
            Ok(value) => Ok(value),
            Err(_) => Err(context
                .borrow_mut()
                .record_serialization_error(JsonSerializationError::new(JsonSerializationErrorKind::InvalidRawValue))),
        },
        Err(crate::lexical::JsonLexicalScanError::Lexical(crate::lexical::JsonLexicalError::Budget(error))) => {
            Err(context.borrow_mut().record_budget_failure(error))
        }
        Err(_) => Err(context
            .borrow_mut()
            .record_serialization_error(JsonSerializationError::new(JsonSerializationErrorKind::InvalidRawValue))),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use qubit_budget::json::JsonContainerKind;
    use qubit_budget::json::JsonResource;
    use qubit_budget::json::JsonValueBudget;
    use qubit_budget::json::JsonValueLimits;
    use serde::ser::SerializeMap;
    use serde::ser::SerializeSeq;
    use serde::ser::SerializeStruct;
    use serde::ser::SerializeStructVariant;
    use serde::ser::SerializeTupleVariant;

    use super::CompoundState;
    use super::JsonValueCompound;
    use super::JsonValueEncodeContext;
    use crate::encode::JsonCollectionKind;
    use crate::encode::JsonSerializationErrorKind;
    use crate::encode::JsonSerializerStateError;

    #[test]
    fn test_compound_count_and_depth_overflow_are_sticky() {
        // Seed only the counters: reaching these guards must not require an
        // allocation or traversal proportional to usize::MAX.
        for path in ["sequence", "tuple-variant", "map", "struct", "struct-variant"] {
            for overflow_count in [true, false] {
                let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
                let mut transaction = budget.transaction();
                let context = RefCell::new(JsonValueEncodeContext::new(&mut transaction, None));
                let mut compound = match path {
                    "sequence" => JsonValueCompound::sequence(&context, 1, JsonContainerKind::Sequence, 0),
                    "tuple-variant" => JsonValueCompound::tuple_variant(&context, "Tuple", 1, 0),
                    "struct-variant" => JsonValueCompound::struct_variant(&context, "Struct", 1, 0),
                    _ => JsonValueCompound::map(&context, 1, 0),
                };
                match &mut compound.state {
                    CompoundState::Sequence { count, depth, .. }
                    | CompoundState::TupleVariant { count, depth, .. }
                    | CompoundState::Map { count, depth, .. }
                    | CompoundState::StructVariant { count, depth, .. } => {
                        if overflow_count {
                            *count = usize::MAX;
                        } else {
                            *depth = usize::MAX;
                        }
                    }
                    _ => unreachable!("fixture is an ordinary compound"),
                }
                let result = match path {
                    "sequence" => SerializeSeq::serialize_element(&mut compound, &true),
                    "tuple-variant" => SerializeTupleVariant::serialize_field(&mut compound, &true),
                    "struct" => SerializeStruct::serialize_field(&mut compound, "key", &true),
                    "struct-variant" => SerializeStructVariant::serialize_field(&mut compound, "key", &true),
                    _ if overflow_count => SerializeMap::serialize_key(&mut compound, "key"),
                    _ => {
                        SerializeMap::serialize_key(&mut compound, "key").expect("key does not increase depth");
                        SerializeMap::serialize_value(&mut compound, &true)
                    }
                };
                let error = result.expect_err("overflow must be rejected before adding a child");
                let expected = if overflow_count {
                    JsonSerializationErrorKind::CollectionLengthOverflow {
                        kind: if matches!(path, "sequence" | "tuple-variant") {
                            JsonCollectionKind::Array
                        } else {
                            JsonCollectionKind::Object
                        },
                    }
                } else {
                    JsonSerializationErrorKind::InvalidSerializerState {
                        reason: JsonSerializerStateError::UnexpectedCompound,
                    }
                };
                assert_eq!(error.kind(), expected, "{path}, count overflow: {overflow_count}");
                assert_eq!(compound.check_failure().expect_err("failure is sticky"), error);
                assert_eq!(context.borrow().ser_error(), error);
            }
        }
    }
}
