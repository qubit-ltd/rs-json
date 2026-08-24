// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serde seed for duplicate-key-free JSON values.

use serde::Deserializer;
use serde::de::DeserializeSeed;

use super::StrictJsonValue;
use super::internal::StrictJsonVisitor;

/// Serde seed that constructs a [`StrictJsonValue`].
///
/// Use this seed when the caller owns a deserializer and needs duplicate-key
/// rejection without requiring an intermediate typed wrapper.
///
/// # Examples
///
/// ```
/// use qubit_json::value::StrictJsonValueSeed;
/// use serde::de::DeserializeSeed;
///
/// let mut deserializer = serde_json::Deserializer::from_str(r#"{"ok":true}"#);
/// let value = StrictJsonValueSeed::new().deserialize(&mut deserializer)?;
/// assert_eq!(value.into_inner()["ok"], true);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct StrictJsonValueSeed;

impl StrictJsonValueSeed {
    /// Creates a seed that rejects duplicate keys in every decoded object.
    ///
    /// # Returns
    ///
    /// A reusable stateless strict JSON value seed.
    #[must_use]
    #[inline(always)]
    pub const fn new() -> Self {
        Self
    }
}

impl<'de> DeserializeSeed<'de> for StrictJsonValueSeed {
    type Value = StrictJsonValue;

    /// Deserializes one JSON value with recursive duplicate-key rejection.
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictJsonVisitor)
    }
}
