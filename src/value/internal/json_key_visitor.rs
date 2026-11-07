// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serde visitor that admits an object key before materializing borrowed text.

use std::fmt;
use std::fmt::Debug;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonValueTransaction;
use serde::de::Error;
use serde::de::Visitor;

/// Visitor that accounts a map key before copying or adopting its text.
///
/// # Type Parameters
///
/// * `'transaction` - Lifetime of the transaction that receives the key charge.
/// * `'budget` - Lifetime of the resource budget backing the transaction.
/// * `R` - Resource identity used by the transaction.
/// * `Q` - Quantity representation accepted by the transaction.
pub(super) struct JsonKeyVisitor<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Transaction receiving the key's resource charge.
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
}

impl<'transaction, 'budget, R, Q> JsonKeyVisitor<'transaction, 'budget, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// Creates a visitor that admits keys into `transaction`.
    ///
    /// # Parameters
    ///
    /// * `transaction` - Mutable transaction to charge when a key is decoded.
    ///
    /// # Returns
    ///
    /// A visitor that borrows `transaction` for the lifetime of that borrow.
    #[must_use]
    #[inline]
    pub(super) fn new(transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>) -> Self {
        Self { transaction }
    }

    /// Admits borrowed text before copying it into an owned string.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used to report admission failures.
    ///
    /// # Parameters
    ///
    /// * `value` - Borrowed key text whose UTF-8 byte length is admitted.
    ///
    /// # Returns
    ///
    /// The copied key text after its byte charge is accepted.
    ///
    /// # Errors
    ///
    /// Returns `E` when the transaction rejects the key's resource charge.
    fn visit_borrowed_text<E>(self, value: &str) -> Result<String, E>
    where
        E: Error,
    {
        self.transaction
            .try_admit(JsonMeasurement::Key { bytes: value.len() })
            .map_err(E::custom)?;
        Ok(value.to_owned())
    }
}

impl<'de, R, Q> Visitor<'de> for JsonKeyVisitor<'_, '_, R, Q>
where
    R: Clone + Debug,
    Q: ResourceQuantity,
{
    /// Owned key text produced after its resource charge is admitted.
    type Value = String;

    /// Describes the string key accepted by this visitor.
    ///
    /// # Parameters
    ///
    /// * `formatter` - Formatter that receives the expected key description.
    ///
    /// # Returns
    ///
    /// The result of writing the expected key description to `formatter`.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object key string")
    }

    /// Admits a borrowed string before copying it.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used to report admission failures.
    ///
    /// # Parameters
    ///
    /// * `value` - Borrowed key text to admit and copy.
    ///
    /// # Returns
    ///
    /// The owned key text after its byte charge is accepted.
    ///
    /// # Errors
    ///
    /// Returns `E` when the transaction rejects the key's resource charge.
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_borrowed_text(value)
    }

    /// Admits a borrowed input string before copying it.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used to report admission failures.
    ///
    /// # Parameters
    ///
    /// * `value` - Input-borrowed key text to admit and copy.
    ///
    /// # Returns
    ///
    /// The owned key text after its byte charge is accepted.
    ///
    /// # Errors
    ///
    /// Returns `E` when the transaction rejects the key's resource charge.
    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_borrowed_text(value)
    }

    /// Admits and adopts an already-owned key string.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Deserialization error type used to report admission failures.
    ///
    /// # Parameters
    ///
    /// * `value` - Owned key text to admit before returning it unchanged.
    ///
    /// # Returns
    ///
    /// The same owned key text after its byte charge is accepted.
    ///
    /// # Errors
    ///
    /// Returns `E` when the transaction rejects the key's resource charge.
    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.transaction
            .try_admit(JsonMeasurement::Key { bytes: value.len() })
            .map_err(E::custom)?;
        Ok(value)
    }
}
