// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Incremental accounting deserialization into a JSON value tree.

use std::fmt::Debug;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonValueTransaction;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde_json::Value;

use super::internal::JsonValueVisitor;

/// Serde seed that constructs a [`Value`] while accounting decoded resources.
///
/// Unlike lexical JSON admission, this seed observes values after a Serde
/// deserializer has decoded them. It is therefore suitable for default budget
/// enforcement inside a type's ordinary [`serde::Deserialize`]
/// implementation, where the original input bytes are unavailable.
pub struct JsonValueSeed<'transaction, 'budget, R, Q = usize>
where
    Q: ResourceQuantity,
{
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
}

impl<'transaction, 'budget, R, Q> JsonValueSeed<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates a root seed using the supplied decoded-value transaction.
    ///
    /// # Parameters
    ///
    /// * `transaction` - Transaction receiving decoded JSON resource charges.
    ///
    /// # Returns
    ///
    /// A seed that constructs one accounted [`Value`] tree.
    #[inline(always)]
    #[must_use]
    pub fn new(
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    ) -> Self {
        Self { transaction }
    }
}

impl<'de, R, Q> DeserializeSeed<'de> for JsonValueSeed<'_, '_, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(JsonValueVisitor::new(self.transaction, 1))
    }
}
