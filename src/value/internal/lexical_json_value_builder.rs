// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Builds a guarded owned JSON value from lexical events.

use std::collections::BTreeSet;

#[cfg(feature = "jcs")]
use serde_json::Number;
use serde_json::Value;

use super::JsonValueFrame;
use super::JsonValueMaterializationError;
use crate::internal::StrictJsonNumber;
use crate::lexical::JsonLexicalConsumer;
use crate::lexical::JsonLexicalEvent;
use crate::lexical::JsonNumberModel;
use crate::value::SafeJsonValue;
use crate::value::internal::SafeValueBuilder;

/// Constructs one owned value from a scanner event sequence.
///
/// The builder keeps materialization failures sticky and exposes a value only
/// after the full event sequence succeeds, so callers cannot receive a partial
/// tree.
pub(crate) struct LexicalJsonValueBuilder {
    /// Number representation contract applied to already admitted tokens.
    model: JsonNumberModel,
    /// Open container frames paired with scanner begin/end events.
    frames: Vec<JsonValueFrame>,
    /// Completed root, populated only after all its children have been built.
    root: Option<SafeJsonValue>,
    /// First materialization failure, retained to prevent later state changes.
    error: Option<JsonValueMaterializationError>,
}
impl LexicalJsonValueBuilder {
    /// Creates an empty value builder.
    #[must_use]
    pub(crate) fn new(model: JsonNumberModel) -> Self {
        Self {
            model,
            frames: Vec::new(),
            root: None,
            error: None,
        }
    }

    /// Consumes one scanner event and returns the first materialization failure
    /// immediately. After a failure, later calls return that retained failure;
    /// the incomplete tree remains private and cannot be finished as a value.
    pub(crate) fn try_accept(&mut self, event: JsonLexicalEvent<'_>) -> Result<(), JsonValueMaterializationError> {
        if let Some(error) = self.error.as_ref() {
            return Err(error.clone());
        }
        let result = self.accept_inner(event);
        if let Err(error) = result {
            self.error = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }

    /// Accepts a number already materialized by a policy-aware caller.
    ///
    /// The caller is responsible for validating the source token and applying
    /// any number policy before this method is called.
    pub(crate) fn try_accept_number(
        &mut self,
        number: serde_json::Number,
        _offset: usize,
    ) -> Result<(), JsonValueMaterializationError> {
        self.try_push_value(Value::Number(number))
    }

    /// Accepts a key already decoded from its scanner-validated source token.
    ///
    /// `offset` is the token's zero-based source byte offset. A materialization
    /// failure is retained, and no partial tree is exposed.
    pub(crate) fn try_accept_decoded_key(
        &mut self,
        key: String,
        offset: usize,
    ) -> Result<(), JsonValueMaterializationError> {
        if let Some(error) = self.error.as_ref() {
            return Err(error.clone());
        }
        let result = self.accept_decoded_key_inner(key, offset);
        if let Err(error) = result {
            self.error = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }

    /// Returns the guarded complete value only if no earlier event failed.
    ///
    /// # Errors
    ///
    /// Returns the first retained materialization failure; in that case the
    /// partially built tree is dropped.
    pub(crate) fn finish_guarded(mut self) -> Result<SafeJsonValue, JsonValueMaterializationError> {
        if let Some(error) = self.error.take() {
            return Err(error);
        }
        debug_assert!(self.frames.is_empty());
        Ok(self.root.take().expect("scanner-complete event stream has one root"))
    }

    /// Applies one scanner-admitted event to the partial tree.
    ///
    /// The scanner guarantees valid JSON token spelling and matched container
    /// boundaries. A failure is retained by `try_accept`; any partially built
    /// children remain guarded and `finish_guarded` drops them without
    /// exposing an incomplete root.
    fn accept_inner(&mut self, event: JsonLexicalEvent<'_>) -> Result<(), JsonValueMaterializationError> {
        match event {
            JsonLexicalEvent::Null { .. } => self.try_push_value(Value::Null),
            JsonLexicalEvent::Boolean { value, .. } => self.try_push_value(Value::Bool(value)),
            JsonLexicalEvent::Number { token, offset } => {
                let number = match self.model {
                    JsonNumberModel::Integer64 => std::str::from_utf8(token)
                        .ok()
                        .and_then(|text| StrictJsonNumber::from_admitted_lexeme(text).ok()),
                    JsonNumberModel::Integer64WithFiniteFallback => std::str::from_utf8(token).ok().and_then(|text| {
                        StrictJsonNumber::from_admitted_lexeme_with_model(
                            text,
                            crate::decode::JsonDecodeNumberModel::Integer64WithFiniteFallback,
                        )
                        .ok()
                    }),
                    JsonNumberModel::SyntaxOnly => None,
                    #[cfg(feature = "jcs")]
                    JsonNumberModel::Binary64 => std::str::from_utf8(token)
                        .ok()
                        .and_then(|text| text.parse::<f64>().ok())
                        .filter(|n| n.is_finite())
                        .and_then(Number::from_f64),
                };
                match number {
                    Some(number) => self.try_push_value(Value::Number(number)),
                    None => Err(JsonValueMaterializationError::InvalidNumber { offset }),
                }
            }
            JsonLexicalEvent::String { token, .. } => {
                let value = serde_json::from_slice::<String>(token).expect("scanner admitted valid JSON string");
                self.try_push_value(Value::String(value))
            }
            JsonLexicalEvent::Key { token, offset } => {
                let key = serde_json::from_slice::<String>(token).expect("scanner admitted valid JSON key");
                self.accept_decoded_key_inner(key, offset)
            }
            JsonLexicalEvent::BeginArray { .. } => {
                self.frames.push(JsonValueFrame::Array(Vec::new()));
                Ok(())
            }
            JsonLexicalEvent::BeginObject { .. } => {
                self.frames.push(JsonValueFrame::Object {
                    values: Vec::new(),
                    keys: BTreeSet::new(),
                    pending_key: None,
                });
                Ok(())
            }
            JsonLexicalEvent::EndArray { .. } => {
                let Some(JsonValueFrame::Array(values)) = self.frames.pop() else {
                    unreachable!("scanner emits matched array events")
                };
                let mut builder = SafeValueBuilder::array();
                for value in values {
                    builder.push(value);
                }
                self.try_push_value(builder.finish())
            }
            JsonLexicalEvent::EndObject { .. } => {
                let Some(JsonValueFrame::Object {
                    values, pending_key, ..
                }) = self.frames.pop()
                else {
                    unreachable!("scanner emits matched object events")
                };
                debug_assert!(pending_key.is_none());
                let mut builder = SafeValueBuilder::object();
                for (key, value) in values {
                    builder.insert(key, value);
                }
                self.try_push_value(builder.finish())
            }
        }
    }

    /// Records a decoded object key after the scanner has admitted its token.
    ///
    /// The current frame must be an object with no pending key. Duplicate
    /// detection uses decoded key equality and retains the second key's source
    /// offset; a successful key remains pending until its value is pushed.
    fn accept_decoded_key_inner(&mut self, key: String, offset: usize) -> Result<(), JsonValueMaterializationError> {
        let Some(JsonValueFrame::Object { keys, pending_key, .. }) = self.frames.last_mut() else {
            unreachable!("scanner emits keys within objects")
        };
        if keys.contains(key.as_str()) {
            return Err(JsonValueMaterializationError::DuplicateKey { offset });
        }
        keys.insert(key.clone());
        *pending_key = Some(key);
        Ok(())
    }

    /// Attaches a complete child to its open parent or installs the root.
    ///
    /// Object parents must have a pending key from a preceding key event.
    /// Every child is guarded as it is stored so an error or unwind can drop a
    /// partial tree safely.
    fn try_push_value(&mut self, value: Value) -> Result<(), JsonValueMaterializationError> {
        let guarded = SafeJsonValue::new(value);
        match self.frames.last_mut() {
            Some(JsonValueFrame::Array(values)) => values.push(guarded),
            Some(JsonValueFrame::Object {
                values, pending_key, ..
            }) => {
                let key = pending_key.take().expect("object value follows key event");
                values.push((key, guarded));
            }
            None => {
                debug_assert!(self.root.is_none());
                self.root = Some(guarded);
            }
        }
        Ok(())
    }
}
impl<'input> JsonLexicalConsumer<'input> for LexicalJsonValueBuilder {
    type Error = JsonValueMaterializationError;

    /// Performs no additional depth check; lexical admission owns that limit.
    fn before_value(&mut self, _depth: usize, _offset: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Performs no additional key check before the key event is decoded.
    fn before_key(&mut self, _offset: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Leaves number-token representation handling to the following event.
    fn before_number(&mut self, _token: &'input str, _offset: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Materializes one admitted event and stops scanning on its first error.
    fn event(&mut self, event: JsonLexicalEvent<'input>) -> Result<(), Self::Error> {
        self.try_accept(event)
    }
}
