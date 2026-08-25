// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serde seed for an object key.

use std::fmt::Debug;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonContainerKind;
use qubit_budget::json::JsonValueTransaction;
use serde::Deserialize;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde::de::Error;

/// A map-key seed that checks the prospective entry before decoding its key.
pub(in crate::value) struct JsonKeySeed<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Transaction receiving the prospective map entry's resource charges.
    pub(super) transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,

    /// Number of entries that would exist after admitting the next key.
    pub(super) prospective: usize,
}

impl<'de, R, Q> DeserializeSeed<'de> for JsonKeySeed<'_, '_, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    type Value = String;

    /// Checks the map-entry limit before delegating key decoding to Serde.
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.transaction
            .check_container_count(JsonContainerKind::Map, self.prospective)
            .map_err(D::Error::custom)?;
        String::deserialize(deserializer)
    }
}
