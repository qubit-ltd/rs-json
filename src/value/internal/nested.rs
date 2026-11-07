// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Protected Serde interoperability for [`crate::value::SafeJsonValue`].
//!
//! Deserialization rejects duplicate object keys and non-finite numbers. Values
//! are assembled through the guarded builder, so an error cannot expose a
//! partially built `SafeJsonValue`. Serialization delegates recursively to the
//! caller's Serde serializer and therefore preserves that serializer's errors.

use serde::Serialize;
use serde::Serializer;
use serde_json::Value;

use crate::value::safe_json_serde::serialize_value;

/// Serializes one borrowed child by re-entering [`serialize_value`].
///
/// The borrow keeps the protected tree owned by its parent throughout the
/// recursive Serde call.
pub(in crate::value) struct Nested<'a>(
    /// Borrows the child value that the parent serializer must serialize.
    pub(in crate::value) &'a Value,
);
impl Serialize for Nested<'_> {
    /// Delegates serialization to the guarded recursive serializer.
    ///
    /// This preserves the serializer's error type while applying the same
    /// protected handling to nested values.
    ///
    /// # Type Parameters
    ///
    /// - `S`: The caller's Serde serializer.
    ///
    /// # Parameters
    ///
    /// - `serializer`: The serializer used for this nested value.
    ///
    /// # Returns
    ///
    /// The serialized representation produced by the recursive serializer.
    ///
    /// # Errors
    ///
    /// Returns the error produced by `serializer` if nested serialization
    /// fails.
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_value(self.0, serializer)
    }
}
