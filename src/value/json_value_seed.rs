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
/// deserializer has decoded them. It cannot inspect original number lexemes or
/// enforce text-level integer and floating-point range rules. Use
/// `JsonDecoder` when decoding JSON text requires those guarantees. This seed
/// remains suitable for decoded-value budget enforcement inside a type's
/// ordinary [`serde::Deserialize`] implementation, where the original input
/// bytes are unavailable.
///
/// # Type Parameters
///
/// * `R` - Resource identity tracked by the value transaction.
/// * `Q` - Quantity representation used for resource accounting.
///
/// # Examples
///
/// ```
/// use qubit_budget::json::{JsonResource, JsonValueBudget, JsonValueLimits};
/// use qubit_json::value::JsonValueSeed;
/// use serde::de::DeserializeSeed;
///
/// let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::default());
/// let mut transaction = budget.transaction();
/// let mut deserializer = serde_json::Deserializer::from_str(r#"{"ok":true}"#);
/// let value = JsonValueSeed::new(&mut transaction).deserialize(&mut deserializer)?;
/// assert_eq!(value["ok"], true);
/// transaction.commit();
/// # Ok::<(), serde_json::Error>(())
/// ```
pub struct JsonValueSeed<'transaction, 'budget, R, Q = usize>
where
    Q: ResourceQuantity,
{
    /// Transaction receiving the decoded value's staged resource charges.
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
    pub fn new(transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>) -> Self {
        Self { transaction }
    }
}

impl<'de, R, Q> DeserializeSeed<'de> for JsonValueSeed<'_, '_, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    type Value = Value;

    /// Builds one value through the accounting visitor.
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(JsonValueVisitor::new(self.transaction, 1))
    }
}
