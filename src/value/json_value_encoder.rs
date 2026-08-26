// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Strict projection from Serde values into materialized JSON values.

use serde::Serialize;
use serde_json::Value;

use self::internal::JsonValueSerializer;
use super::JsonValueEncodeError;

mod internal;

/// Projects serializable values into strict materialized JSON.
///
/// The encoder accepts the full signed and unsigned 64-bit JSON integer range,
/// rejects wider numeric values and non-finite floats, validates map keys, and
/// rejects duplicate object keys. It owns no resource budget; use
/// [`crate::encode::JsonEncoder`] when encoded text and resource accounting are
/// required.
///
/// # Examples
///
/// ```
/// use qubit_json::value::JsonValueEncoder;
/// use serde_json::json;
///
/// let encoder = JsonValueEncoder::new();
/// let value = encoder.encode(&json!({"ok": true}))?;
/// assert_eq!(value, json!({"ok": true}));
/// # Ok::<(), qubit_json::value::JsonValueEncodeError>(())
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonValueEncoder {
    /// Prevents external struct literals while reserving room for future
    /// policy.
    private: (),
}

impl JsonValueEncoder {
    /// Creates a strict encoder with the default immutable policy.
    ///
    /// # Returns
    ///
    /// A reusable encoder that performs no resource accounting.
    #[must_use]
    #[inline(always)]
    pub const fn new() -> Self {
        Self { private: () }
    }

    /// Projects one serializable value into strict JSON.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Source type traversed through its Serde representation.
    ///
    /// # Parameters
    ///
    /// * `value` - Borrowed value to project without taking ownership.
    ///
    /// # Returns
    ///
    /// The fully materialized JSON value.
    ///
    /// # Errors
    ///
    /// Returns [`JsonValueEncodeError::NonFiniteFloat`] for any direct or
    /// nested non-finite float. Returns
    /// [`JsonValueEncodeError::Serialization`] for unsupported Serde shapes,
    /// out-of-range numeric values, invalid raw JSON, or duplicate object keys.
    pub fn encode<T>(&self, value: &T) -> Result<Value, JsonValueEncodeError>
    where
        T: Serialize + ?Sized,
    {
        let () = self.private;
        value.serialize(JsonValueSerializer)
    }
}
