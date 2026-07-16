// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the coarse top-level JSON kinds used by constrained decode methods.

use std::{
    fmt,
    str::FromStr,
};

use serde_json::Value;

/// Represents the top-level kind of a parsed JSON value.
///
/// The decoder uses this type to report whether the parsed value is an object,
/// an array, or any other scalar-like JSON value.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonTopLevelKind {
    /// Indicates that the parsed top-level value is a JSON object.
    Object,
    /// Indicates that the parsed top-level value is a JSON array.
    Array,
    /// Indicates that the parsed top-level value is neither an object nor an
    /// array.
    Other,
}

impl JsonTopLevelKind {
    /// Classifies the top-level kind of `value`.
    ///
    /// This helper is used internally by constrained decode methods and may
    /// also be useful to callers inspecting decoded [`Value`] instances.
    ///
    /// # Arguments
    ///
    /// * `value` - JSON value to classify.
    ///
    /// # Returns
    ///
    /// [`Self::Object`] for objects, [`Self::Array`] for arrays, and
    /// [`Self::Other`] for scalar-like values.
    #[inline]
    #[must_use]
    pub fn of(value: &Value) -> Self {
        match value {
            Value::Object(_) => Self::Object,
            Value::Array(_) => Self::Array,
            _ => Self::Other,
        }
    }

    /// Classifies validated normalized JSON text by its first JSON token.
    ///
    /// # Arguments
    ///
    /// * `value` - Normalized JSON text to inspect.
    ///
    /// # Returns
    ///
    /// [`Self::Object`] when the first token is `{`, [`Self::Array`] when it is
    /// `[`, and [`Self::Other`] otherwise.
    #[inline]
    #[must_use]
    pub(crate) fn of_normalized_json(value: &str) -> Self {
        match value
            .bytes()
            .find(|byte| !matches!(*byte, b' ' | b'\n' | b'\r' | b'\t'))
        {
            Some(b'{') => Self::Object,
            Some(b'[') => Self::Array,
            _ => Self::Other,
        }
    }
}

impl From<&Value> for JsonTopLevelKind {
    /// Classifies a borrowed dynamic JSON value.
    ///
    /// # Arguments
    ///
    /// * `value` - JSON value to classify.
    ///
    /// # Returns
    ///
    /// The value's coarse top-level kind.
    #[inline(always)]
    fn from(value: &Value) -> Self {
        Self::of(value)
    }
}

impl fmt::Display for JsonTopLevelKind {
    /// Writes the stable lowercase name of this top-level kind.
    ///
    /// # Arguments
    ///
    /// * `f` - Destination formatter.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the kind name is written successfully.
    ///
    /// # Errors
    ///
    /// Returns a formatting error when the destination rejects the write.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Object => "object",
            Self::Array => "array",
            Self::Other => "other",
        };
        f.write_str(name)
    }
}

impl FromStr for JsonTopLevelKind {
    type Err = &'static str;

    /// Parses a top-level kind name without ASCII case sensitivity.
    ///
    /// # Arguments
    ///
    /// * `value` - Kind name to parse.
    ///
    /// # Returns
    ///
    /// The matching top-level kind.
    ///
    /// # Errors
    ///
    /// Returns a static diagnostic when `value` is not a known kind name.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.eq_ignore_ascii_case("object") {
            Ok(Self::Object)
        } else if value.eq_ignore_ascii_case("array") {
            Ok(Self::Array)
        } else if value.eq_ignore_ascii_case("other") {
            Ok(Self::Other)
        } else {
            Err("unknown JsonTopLevelKind")
        }
    }
}
