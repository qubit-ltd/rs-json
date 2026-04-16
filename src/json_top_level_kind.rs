/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Defines the coarse top-level JSON kinds used by constrained decode methods.
//!
//! Author: Haixing Hu

use parse_display::{Display, FromStr as DeriveFromStr};

use serde_json::Value;

/// Represents the top-level kind of a parsed JSON value.
///
/// The decoder uses this type to report whether the parsed value is an object,
/// an array, or any other scalar-like JSON value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, DeriveFromStr)]
#[display(style = "snake_case")]
pub enum JsonTopLevelKind {
    /// Indicates that the parsed top-level value is a JSON object.
    #[from_str(regex = "(?i)object")]
    Object,
    /// Indicates that the parsed top-level value is a JSON array.
    #[from_str(regex = "(?i)array")]
    Array,
    /// Indicates that the parsed top-level value is neither an object nor an
    /// array.
    #[from_str(regex = "(?i)other")]
    Other,
}

impl JsonTopLevelKind {
    /// Classifies the top-level kind of `value`.
    ///
    /// This helper is used internally by constrained decode methods and may
    /// also be useful to callers inspecting decoded [`Value`] instances.
    #[inline]
    #[must_use]
    pub fn of(value: &Value) -> Self {
        match value {
            Value::Object(_) => Self::Object,
            Value::Array(_) => Self::Array,
            _ => Self::Other,
        }
    }
}

impl From<&Value> for JsonTopLevelKind {
    #[inline]
    fn from(value: &Value) -> Self {
        Self::of(value)
    }
}
