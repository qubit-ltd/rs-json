// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Failure categories retained during JSON value projection.

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;

use crate::encode::JsonSerializationError;

/// The first failure retained while projecting one JSON value.
///
/// It records a measured budget rejection, a structured serialization error,
/// or a private payload exceeding its per-payload UTF-8 byte limit. Serde may
/// receive an opaque error to stop traversal; the owning encoder recovers the
/// retained failure and its original category.
///
/// # Type Parameters
///
/// - `R`: Resource type measured by the budget transaction.
/// - `Q`: Quantity type used for measured budget errors; it must implement
///   [`ResourceQuantity`].
pub(in crate::value::json_value_encoder) enum JsonValueEncodeFailure<R, Q>
where
    Q: ResourceQuantity,
{
    /// A measured budget rejected a value, container, or collection count.
    Budget(#[doc = "Measured resource rejection retained for the owning encoder."] MeasuredBudgetError<R, Q>),
    /// Serde or the projection serializer reported a structured error.
    Serialization(#[doc = "Structured serialization error preserved for the owning encoder."] JsonSerializationError),
    /// A private payload exceeded its inclusive UTF-8 byte threshold.
    PrivateTextLimitExceeded {
        /// Inclusive UTF-8 byte threshold configured for each private payload.
        maximum: usize,
    },
}
