// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::fmt;

use serde_json::Value;

pub(in crate::value) fn values_equal(left: &Value, right: &Value) -> bool {
    let mut pending = vec![(left, right)];
    while let Some((left, right)) = pending.pop() {
        match (left, right) {
            (Value::Null, Value::Null) => {}
            (Value::Bool(a), Value::Bool(b)) if a == b => {}
            (Value::Number(a), Value::Number(b)) if a == b => {}
            (Value::String(a), Value::String(b)) if a == b => {}
            (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
                pending.extend(a.iter().zip(b));
            }
            (Value::Object(a), Value::Object(b)) if a.len() == b.len() => {
                for (key, value) in a {
                    let Some(other_value) = b.get(key) else {
                        return false;
                    };
                    pending.push((value, other_value));
                }
            }
            _ => return false,
        }
    }
    true
}

pub(in crate::value) fn debug_value(value: &Value, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    enum Action<'a> {
        Value(&'a Value),
        Text(&'static str),
        Key(&'a str),
    }
    let mut actions = vec![Action::Value(value)];
    while let Some(action) = actions.pop() {
        match action {
            Action::Text(text) => formatter.write_str(text)?,
            Action::Key(key) => fmt::Debug::fmt(key, formatter)?,
            Action::Value(Value::Null) => formatter.write_str("Null")?,
            Action::Value(Value::Bool(value)) => write!(formatter, "Bool({value:?})")?,
            Action::Value(Value::Number(value)) => write!(formatter, "Number({value:?})")?,
            Action::Value(Value::String(value)) => write!(formatter, "String({value:?})")?,
            Action::Value(Value::Array(values)) => {
                formatter.write_str("Array [")?;
                actions.push(Action::Text("]"));
                for (index, child) in values.iter().enumerate().rev() {
                    if index + 1 < values.len() {
                        actions.push(Action::Text(", "));
                    }
                    actions.push(Action::Value(child));
                }
            }
            Action::Value(Value::Object(values)) => {
                formatter.write_str("Object {")?;
                actions.push(Action::Text("}"));
                for (index, (key, child)) in values.iter().enumerate().rev() {
                    if index + 1 < values.len() {
                        actions.push(Action::Text(", "));
                    }
                    actions.push(Action::Value(child));
                    actions.push(Action::Text(": "));
                    actions.push(Action::Key(key));
                }
            }
        }
    }
    Ok(())
}
