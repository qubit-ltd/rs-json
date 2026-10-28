// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::fmt;

use serde_json::Value;

use super::JsonValueCompareFrame;
use super::JsonValueDebugAction;

/// Compares two JSON values without recursion or expansion of all siblings.
///
/// Objects are compared by key lookup, independent of iteration order. The
/// explicit continuation stack uses O(depth) auxiliary space, excluding both
/// input trees.
pub(in crate::value) fn values_equal(left: &Value, right: &Value) -> bool {
    let mut current = Some((left, right));
    let mut frames = Vec::new();
    loop {
        if let Some((left, right)) = current.take() {
            match (left, right) {
                (Value::Null, Value::Null) => {}
                (Value::Bool(a), Value::Bool(b)) if a == b => {}
                (Value::Number(a), Value::Number(b)) if a == b => {}
                (Value::String(a), Value::String(b)) if a == b => {}
                (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
                    let mut frame = JsonValueCompareFrame::Array {
                        left: a.iter(),
                        right: b.iter(),
                    };
                    match frame.next_pair() {
                        Some(Some(pair)) => {
                            frames.push(frame);
                            current = Some(pair);
                        }
                        None => {}
                        Some(None) => unreachable!("array frames do not perform key lookup"),
                    }
                }
                (Value::Object(a), Value::Object(b)) if a.len() == b.len() => {
                    let mut frame = JsonValueCompareFrame::Object {
                        left: a.iter(),
                        right: b,
                    };
                    match frame.next_pair() {
                        Some(Some(pair)) => {
                            frames.push(frame);
                            current = Some(pair);
                        }
                        None => {}
                        Some(None) => return false,
                    }
                }
                _ => return false,
            }
        } else if let Some(frame) = frames.last_mut() {
            match frame.next_pair() {
                Some(Some(pair)) => current = Some(pair),
                None => {
                    frames.pop();
                }
                Some(None) => return false,
            }
        } else {
            return true;
        }
    }
}

/// Formats a JSON value iteratively, preserving the owner/view Debug syntax.
///
/// Object entries follow the map's iteration order. The formatter's first
/// error is returned immediately. The action stack uses O(depth) auxiliary
/// space, excluding the input value and any output storage owned by the
/// formatter.
pub(in crate::value) fn debug_value(value: &Value, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut actions = vec![JsonValueDebugAction::Value(value)];
    while let Some(action) = actions.pop() {
        match action {
            JsonValueDebugAction::Text(text) => formatter.write_str(text)?,
            JsonValueDebugAction::Key(key) => fmt::Debug::fmt(key, formatter)?,
            JsonValueDebugAction::Value(Value::Null) => formatter.write_str("Null")?,
            JsonValueDebugAction::Value(Value::Bool(value)) => write!(formatter, "Bool({value:?})")?,
            JsonValueDebugAction::Value(Value::Number(value)) => write!(formatter, "Number({value:?})")?,
            JsonValueDebugAction::Value(Value::String(value)) => write!(formatter, "String({value:?})")?,
            JsonValueDebugAction::Value(Value::Array(values)) => {
                formatter.write_str("Array [")?;
                actions.push(JsonValueDebugAction::Array {
                    values: values.iter(),
                    has_emitted_item: false,
                });
            }
            JsonValueDebugAction::Value(Value::Object(values)) => {
                formatter.write_str("Object {")?;
                actions.push(JsonValueDebugAction::Object {
                    values: values.iter(),
                    has_emitted_item: false,
                });
            }
            JsonValueDebugAction::Array {
                mut values,
                has_emitted_item,
            } => {
                if let Some(value) = values.next() {
                    if has_emitted_item {
                        formatter.write_str(", ")?;
                    }
                    actions.push(JsonValueDebugAction::Array {
                        values,
                        has_emitted_item: true,
                    });
                    actions.push(JsonValueDebugAction::Value(value));
                } else {
                    formatter.write_str("]")?;
                }
            }
            JsonValueDebugAction::Object {
                mut values,
                has_emitted_item,
            } => {
                if let Some((key, value)) = values.next() {
                    if has_emitted_item {
                        formatter.write_str(", ")?;
                    }
                    actions.push(JsonValueDebugAction::Object {
                        values,
                        has_emitted_item: true,
                    });
                    actions.push(JsonValueDebugAction::Value(value));
                    actions.push(JsonValueDebugAction::Text(": "));
                    actions.push(JsonValueDebugAction::Key(key));
                } else {
                    formatter.write_str("}")?;
                }
            }
        }
    }
    Ok(())
}
