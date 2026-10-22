// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serde seed that constructs JCS binary64 values and rejects duplicate keys.

use std::cell::Cell;

use serde::Deserializer;
use serde::de::DeserializeSeed;

use super::jcs_value_visitor::JcsValueVisitor;

/// Seed for an owned JSON value with one operation-scoped duplicate marker.
pub(in crate::decode) struct JcsValueSeed<'a> {
    pub(in crate::decode) duplicate_key: &'a Cell<bool>,
}

impl<'de> DeserializeSeed<'de> for JcsValueSeed<'_> {
    type Value = serde_json::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(JcsValueVisitor {
            duplicate_key: self.duplicate_key,
        })
    }
}
