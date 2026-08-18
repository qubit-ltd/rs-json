// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serde seed for a JSON container child.

use std::fmt::Debug;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonContainerKind;
use qubit_budget::json::JsonValueTransaction;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde::de::Error;
use serde_json::Value;

use super::JsonValueVisitor;

/// A child seed that rejects an excess container member before materialization.
pub(in crate::value) struct JsonValueChildSeed<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    pub(super) transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,

    pub(super) depth: usize,

    pub(super) kind: JsonContainerKind,

    pub(super) prospective: usize,
}

impl<'de, R, Q> DeserializeSeed<'de> for JsonValueChildSeed<'_, '_, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.transaction
            .check_container_count(self.kind, self.prospective)
            .map_err(D::Error::custom)?;
        deserializer.deserialize_any(JsonValueVisitor::new(self.transaction, self.depth))
    }
}
