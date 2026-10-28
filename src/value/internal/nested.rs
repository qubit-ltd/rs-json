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
pub(in crate::value) struct Nested<'a>(pub(in crate::value) &'a Value);
impl Serialize for Nested<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_value(self.0, serializer)
    }
}
