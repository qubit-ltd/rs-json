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
/// Do not pass this seed to [`crate::decode::JsonDecoder::decode_seed_str`] or
/// [`crate::decode::JsonDecoder::decode_seed_utf8`] when its transaction and
/// the decoder represent the same logical decoded-value budget. `JsonDecoder`
/// already accounts the complete value during lexical admission, so the seed
/// would charge that value a second time. In that pipeline, use a seed that
/// performs only domain deserialization and domain-specific checks.
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
/// use qubit_json::value::AccountingJsonValueSeed;
/// use serde::de::DeserializeSeed;
///
/// let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::default());
/// let mut transaction = budget.transaction();
/// let mut deserializer = serde_json::Deserializer::from_str(r#"{"ok":true}"#);
/// let value = AccountingJsonValueSeed::new(&mut transaction).deserialize(&mut deserializer)?;
/// assert_eq!(value["ok"], true);
/// transaction.commit();
/// # Ok::<(), serde_json::Error>(())
/// ```
pub struct AccountingJsonValueSeed<'transaction, 'budget, R, Q = usize>
where
    Q: ResourceQuantity,
{
    /// Transaction receiving the decoded value's staged resource charges.
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
}

impl<'transaction, 'budget, R, Q> AccountingJsonValueSeed<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates a root seed using the supplied decoded-value transaction.
    ///
    /// # Parameters
    ///
    /// * `transaction` - Transaction receiving decoded JSON resource charges.
    ///   It must not duplicate decoded-value accounting already performed by an
    ///   outer [`crate::decode::JsonDecoder`].
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

impl<'de, R, Q> DeserializeSeed<'de> for AccountingJsonValueSeed<'_, '_, R, Q>
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
