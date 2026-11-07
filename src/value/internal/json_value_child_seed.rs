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
///
/// # Type Parameters
///
/// * `'transaction` - Lifetime of the transaction receiving resource charges.
/// * `'budget` - Lifetime of the resource budget backing that transaction.
/// * `R` - Resource identity used for budget accounting.
/// * `Q` - Quantity representation used for budget accounting; it must
///   implement [`ResourceQuantity`].
pub(in crate::value) struct JsonValueChildSeed<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Transaction receiving the child value's resource charges.
    pub(super) transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,

    /// Root-inclusive depth assigned to the child value.
    pub(super) depth: usize,

    /// Container kind whose member count is checked before decoding.
    pub(super) kind: JsonContainerKind,

    /// Number of members that would exist after admitting this child.
    pub(super) prospective: usize,

    /// Whether duplicate decoded object keys are rejected before their values.
    pub(super) reject_duplicate_keys: bool,
}

impl<'de, R, Q> DeserializeSeed<'de> for JsonValueChildSeed<'_, '_, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// JSON value materialized by the child visitor.
    type Value = Value;

    /// Checks the container limit and then delegates construction to the child
    /// visitor.
    ///
    /// # Type Parameters
    ///
    /// * `D` - Deserializer supplying the child's JSON representation.
    ///
    /// # Parameters
    ///
    /// * `self` - Seed carrying the transaction, depth, and container checks.
    /// * `deserializer` - Source from which the child value is decoded.
    ///
    /// # Returns
    ///
    /// The materialized JSON value after the container limit check succeeds.
    ///
    /// # Errors
    ///
    /// Returns the deserializer's error if the prospective container count
    /// exceeds its limit or if the child representation cannot be decoded.
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.transaction
            .check_container_count(self.kind, self.prospective)
            .map_err(D::Error::custom)?;
        deserializer.deserialize_any(JsonValueVisitor::new(
            self.transaction,
            self.depth,
            self.reject_duplicate_keys,
        ))
    }
}
