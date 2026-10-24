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
    #[inline]
    pub(super) fn new(transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>) -> Self {
        Self { transaction }
    }

    /// Admits borrowed text before copying it into an owned string.
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
    type Value = String;

    /// Describes the string key accepted by this visitor.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object key string")
    }

    /// Admits a borrowed string before copying it.
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_borrowed_text(value)
    }

    /// Admits a borrowed input string before copying it.
    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_borrowed_text(value)
    }

    /// Admits and adopts an already-owned key string.
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
