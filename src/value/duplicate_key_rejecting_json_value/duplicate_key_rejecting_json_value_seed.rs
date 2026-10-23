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

use super::DuplicateKeyRejectingJsonValue;
use super::internal::DuplicateKeyRejectingJsonVisitor;

/// Serde seed that constructs a [`DuplicateKeyRejectingJsonValue`].
///
/// Use this seed when the caller owns a deserializer and needs duplicate-key
/// rejection without requiring an intermediate typed wrapper.
///
/// Native `i64`, `u64`, and finite `f64` events retain their numeric
/// representation. If an upstream deserializer exposes an arbitrary-precision
/// number as map events, this seed treats those events as an ordinary object
/// because it cannot determine their provenance. Lexical admission by
/// [`crate::decode::JsonDecoder`] does not change the subsequent Serde event
/// representation. See [`DuplicateKeyRejectingJsonValue`] for the corresponding
/// wrapper behavior and `doc/number_contract.md` in the repository for the
/// numeric contract.
///
/// # Examples
///
/// ```
/// use qubit_json::value::DuplicateKeyRejectingJsonValueSeed;
/// use serde::de::DeserializeSeed;
///
/// let mut deserializer = serde_json::Deserializer::from_str(r#"{"ok":true}"#);
/// let value = DuplicateKeyRejectingJsonValueSeed::new().deserialize(&mut deserializer)?;
/// assert_eq!(value.into_inner()["ok"], true);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct DuplicateKeyRejectingJsonValueSeed;

impl DuplicateKeyRejectingJsonValueSeed {
    /// Creates a seed that rejects duplicate keys in every decoded object.
    ///
    /// # Returns
    ///
    /// A reusable stateless duplicate-key-rejecting JSON value seed.
    #[must_use]
    #[inline(always)]
    pub const fn new() -> Self {
        Self
    }
}

impl<'de> DeserializeSeed<'de> for DuplicateKeyRejectingJsonValueSeed {
    type Value = DuplicateKeyRejectingJsonValue;

    /// Deserializes one JSON value with recursive duplicate-key rejection.
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateKeyRejectingJsonVisitor)
    }
}
