// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Builds an owned JSON value from admitted lexical events.

#[cfg(feature = "jcs")]
use serde_json::Number;
use serde_json::Value;

use super::JsonValueFrame;
use super::JsonValueMaterializationError;
use crate::internal::StrictJsonNumber;
use crate::lexical::JsonLexicalEvent;
use crate::lexical::JsonNumberModel;

/// Fixed maximum number of nested containers in a materialized JSON value.
///
/// RawValue projections accept container depths 1 through 127 and reject 128.
const MAX_MATERIALIZATION_DEPTH: usize = 127;

/// Constructs one owned value from a complete scanner event sequence.
pub(crate) struct LexicalJsonValueBuilder {
    /// Numeric representation selected for materialized values.
    model: JsonNumberModel,
    /// Open array and object frames awaiting completion events.
    frames: Vec<JsonValueFrame>,
    /// Completed top-level value, populated by the root event.
    root: Option<Value>,
    /// First materialization error, after which incoming events are ignored.
    error: Option<JsonValueMaterializationError>,
}

impl LexicalJsonValueBuilder {
    /// Creates an empty value builder.
    ///
    /// # Parameters
    ///
    /// * `model` - Numeric representation used for admitted number tokens.
    ///
    /// # Returns
    ///
    /// A builder ready to consume lexical events.
    #[inline]
    #[must_use]
    pub(crate) fn new(model: JsonNumberModel) -> Self {
        Self {
            model,
            frames: Vec::new(),
            root: None,
            error: None,
        }
    }

    /// Consumes one event from a document admitted by the lexical scanner.
    ///
    /// # Parameters
    ///
    /// * `event` - Next event in the scanner's admitted document sequence.
    ///
    /// # Panics
    ///
    /// Panics when an event violates scanner guarantees, such as an invalid
    /// string token, a key outside an object, or a mismatched container end.
    pub(crate) fn accept(&mut self, event: JsonLexicalEvent<'_>) {
        if self.error.is_some() {
            return;
        }
        match event {
            JsonLexicalEvent::Null { offset } => {
                let _ = offset;
                self.push_value(Value::Null);
            }
            JsonLexicalEvent::Boolean { value, offset } => {
                let _ = offset;
                self.push_value(Value::Bool(value));
            }
            JsonLexicalEvent::Number { token, offset } => {
                let _ = offset;
                let number = match self.model {
                    JsonNumberModel::Integer64 => std::str::from_utf8(token)
                        .ok()
                        .and_then(|text| StrictJsonNumber::from_admitted_lexeme(text).ok()),
                    #[cfg(feature = "jcs")]
                    JsonNumberModel::Binary64 => std::str::from_utf8(token)
                        .ok()
                        .and_then(|text| text.parse::<f64>().ok())
                        .filter(|value| value.is_finite())
                        .and_then(Number::from_f64),
                };
                if let Some(number) = number {
                    self.push_value(Value::Number(number));
                } else {
                    self.error = Some(JsonValueMaterializationError::InvalidNumber { offset });
                }
            }
            JsonLexicalEvent::String { token, offset } => {
                let _ = offset;
                let value = serde_json::from_slice::<String>(token).expect("admitted JSON string must decode");
                self.push_value(Value::String(value));
            }
            JsonLexicalEvent::Key { token, offset } => {
                let key = serde_json::from_slice::<String>(token).expect("admitted JSON key must decode");
                let Some(JsonValueFrame::Object { values, pending_key }) = self.frames.last_mut() else {
                    unreachable!("scanner emits keys only inside objects");
                };
                if values.contains_key(&key) {
                    self.error = Some(JsonValueMaterializationError::DuplicateKey { offset });
                } else {
                    *pending_key = Some(key);
                }
            }
            JsonLexicalEvent::BeginArray { offset } => {
                if self.check_depth(offset) {
                    self.frames.push(JsonValueFrame::Array(Vec::new()));
                }
            }
            JsonLexicalEvent::BeginObject { offset } => {
                if self.check_depth(offset) {
                    self.frames.push(JsonValueFrame::Object {
                        values: serde_json::Map::new(),
                        pending_key: None,
                    });
                }
            }
            JsonLexicalEvent::EndArray { offset } => {
                let _ = offset;
                let Some(JsonValueFrame::Array(values)) = self.frames.pop() else {
                    unreachable!("scanner emits matching array events");
                };
                self.push_value(Value::Array(values));
            }
            JsonLexicalEvent::EndObject { offset } => {
                let _ = offset;
                let Some(JsonValueFrame::Object { values, pending_key }) = self.frames.pop() else {
                    unreachable!("scanner emits matching object events");
                };
                debug_assert!(pending_key.is_none());
                self.push_value(Value::Object(values));
            }
        }
    }

    /// Returns the complete value or the first materialization failure.
    ///
    /// # Returns
    ///
    /// The completed JSON value when every event was materialized successfully.
    ///
    /// # Errors
    ///
    /// Returns the first recorded error when an admitted number cannot be
    /// represented, an object key is duplicated, or the container depth limit
    /// is exceeded.
    ///
    /// # Panics
    ///
    /// Panics when the event sequence is incomplete and has not produced a
    /// root value.
    pub(crate) fn finish(self) -> Result<Value, JsonValueMaterializationError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        debug_assert!(self.frames.is_empty());
        Ok(self.root.expect("complete admitted event stream has a root value"))
    }

    /// Rejects a container event when another frame would exceed the fixed
    /// limit.
    ///
    /// # Parameters
    ///
    /// * `offset` - Byte offset of the container's opening token.
    ///
    /// # Returns
    ///
    /// `true` when another frame fits within the limit, or `false` after
    /// recording a depth-limit error.
    #[must_use]
    fn check_depth(&mut self, offset: usize) -> bool {
        if self.frames.len() == MAX_MATERIALIZATION_DEPTH {
            self.error = Some(JsonValueMaterializationError::DepthLimit {
                offset,
                limit: MAX_MATERIALIZATION_DEPTH,
            });
            false
        } else {
            true
        }
    }

    /// Attaches a completed value to its parent frame or stores it as the root.
    ///
    /// # Parameters
    ///
    /// * `value` - Fully materialized value to attach to the current frame.
    fn push_value(&mut self, value: Value) {
        match self.frames.last_mut() {
            Some(JsonValueFrame::Array(values)) => values.push(value),
            Some(JsonValueFrame::Object { values, pending_key }) => {
                let key = pending_key.take().expect("object value must follow a key event");
                values.insert(key, value);
            }
            None => {
                debug_assert!(self.root.is_none());
                self.root = Some(value);
            }
        }
    }
}
