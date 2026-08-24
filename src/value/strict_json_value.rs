// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deserializes JSON values while rejecting duplicate object keys.

use serde::Deserialize;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde_json::Value;

mod internal;
mod strict_json_value_seed;

pub use strict_json_value_seed::StrictJsonValueSeed;

/// A JSON value whose deserialization rejects duplicate object keys.
///
/// This wrapper is useful for document formats where accepting the usual
/// last-key-wins object behavior would make the input ambiguous. It preserves
/// serde_json's standard `i64`, `u64`, and finite `f64` number representation
/// while recursively validating every object. For raw JSON text, use
/// [`crate::decode::JsonDecoder`] when the crate's explicit numeric range
/// contract must be enforced before deserialization.
///
/// # Examples
///
/// ```
/// use qubit_json::value::StrictJsonValue;
///
/// let value: StrictJsonValue = serde_json::from_str(r#"{"ok":true}"#)?;
/// assert_eq!(value.into_inner()["ok"], true);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, PartialEq)]
pub struct StrictJsonValue(Value);

impl StrictJsonValue {
    /// Returns the validated JSON value.
    ///
    /// # Returns
    ///
    /// The owned serde_json value constructed during deserialization.
    #[must_use]
    #[inline(always)]
    pub fn into_inner(self) -> Value {
        self.0
    }
}

impl<'de> Deserialize<'de> for StrictJsonValue {
    /// Deserializes JSON recursively while rejecting duplicate object keys.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        StrictJsonValueSeed::new().deserialize(deserializer)
    }
}
