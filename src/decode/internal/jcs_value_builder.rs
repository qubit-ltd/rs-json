// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Builds an owned JCS value from admitted lexical events.

use serde_json::Number;
use serde_json::Value;

use super::JcsMaterializationError;
use super::JcsValueFrame;
use crate::lexical::JsonLexicalEvent;

/// Fixed maximum number of nested containers in a materialized JCS value.
const MAX_MATERIALIZATION_DEPTH: usize = 127;

/// Constructs one owned value from a complete scanner event sequence.
pub(in crate::decode) struct JcsValueBuilder {
    frames: Vec<JcsValueFrame>,
    root: Option<Value>,
    error: Option<JcsMaterializationError>,
}

impl JcsValueBuilder {
    /// Creates an empty value builder.
    #[inline]
    #[must_use]
    pub(in crate::decode) fn new() -> Self {
        Self {
            frames: Vec::new(),
            root: None,
            error: None,
        }
    }

    /// Consumes one event from a document admitted by the lexical scanner.
    pub(in crate::decode) fn accept(&mut self, event: JsonLexicalEvent<'_>) {
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
                let token = std::str::from_utf8(token).expect("admitted number token is ASCII");
                let binary64 = token.parse::<f64>().expect("admitted JCS number is finite binary64");
                let number = Number::from_f64(binary64).expect("admission rejected non-finite numbers");
                self.push_value(Value::Number(number));
            }
            JsonLexicalEvent::String { token, offset } => {
                let _ = offset;
                let value = serde_json::from_slice::<String>(token).expect("admitted JSON string must decode");
                self.push_value(Value::String(value));
            }
            JsonLexicalEvent::Key { token, offset } => {
                let key = serde_json::from_slice::<String>(token).expect("admitted JSON key must decode");
                let Some(JcsValueFrame::Object { values, pending_key }) = self.frames.last_mut() else {
                    unreachable!("scanner emits keys only inside objects");
                };
                if values.contains_key(&key) {
                    self.error = Some(JcsMaterializationError::DuplicateKey { offset });
                } else {
                    *pending_key = Some(key);
                }
            }
            JsonLexicalEvent::BeginArray { offset } => {
                if self.check_depth(offset) {
                    self.frames.push(JcsValueFrame::Array(Vec::new()));
                }
            }
            JsonLexicalEvent::BeginObject { offset } => {
                if self.check_depth(offset) {
                    self.frames.push(JcsValueFrame::Object {
                        values: serde_json::Map::new(),
                        pending_key: None,
                    });
                }
            }
            JsonLexicalEvent::EndArray { offset } => {
                let _ = offset;
                let Some(JcsValueFrame::Array(values)) = self.frames.pop() else {
                    unreachable!("scanner emits matching array events");
                };
                self.push_value(Value::Array(values));
            }
            JsonLexicalEvent::EndObject { offset } => {
                let _ = offset;
                let Some(JcsValueFrame::Object { values, pending_key }) = self.frames.pop() else {
                    unreachable!("scanner emits matching object events");
                };
                debug_assert!(pending_key.is_none());
                self.push_value(Value::Object(values));
            }
        }
    }

    /// Returns the complete value or the first materialization failure.
    pub(in crate::decode) fn finish(self) -> Result<Value, JcsMaterializationError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        debug_assert!(self.frames.is_empty());
        Ok(self.root.expect("complete admitted event stream has a root value"))
    }

    fn check_depth(&mut self, offset: usize) -> bool {
        if self.frames.len() == MAX_MATERIALIZATION_DEPTH {
            self.error = Some(JcsMaterializationError::DepthLimit {
                offset,
                limit: MAX_MATERIALIZATION_DEPTH,
            });
            false
        } else {
            true
        }
    }

    fn push_value(&mut self, value: Value) {
        match self.frames.last_mut() {
            Some(JcsValueFrame::Array(values)) => values.push(value),
            Some(JcsValueFrame::Object { values, pending_key }) => {
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
