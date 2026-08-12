// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines errors returned by the budget-aware JSON/Serde adapters.
// qubit-style: allow source-test-pair

use std::fmt;

use qubit_budget::BudgetError;
use qubit_budget::MeasuredBudgetError;
use qubit_budget::QuantityConversionError;
use serde_json::Error as JsonError;
use thiserror::Error;

use super::JsonSyntaxError;

/// Errors returned by budget-aware JSON/Serde adapters.
#[must_use]
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum JsonSerdeError<R, Q = usize>
where
    Q: Copy + fmt::Debug,
{
    /// The document exceeded one configured resource budget.
    #[error("JSON resource budget exceeded: {0}")]
    Budget(#[source] BudgetError<R, Q>),

    /// A native measurement cannot be represented by the configured quantity.
    #[error("JSON resource quantity conversion failed: {source}")]
    Quantity {
        /// Resource whose accounting required the conversion.
        resource: R,

        /// Failed quantity conversion.
        #[source]
        source: QuantityConversionError,
    },

    /// The document failed non-recursive JSON lexical admission.
    #[error(transparent)]
    Syntax(#[from] JsonSyntaxError),

    /// Serde JSON rejected the document or value.
    #[error("JSON/Serde processing error: {0}")]
    Json(#[source] JsonError),

    /// The destination writer rejected serialized bytes.
    #[error("JSON output writer failed: {0}")]
    Io(#[source] std::io::Error),
}

impl<R, Q> From<MeasuredBudgetError<R, Q>> for JsonSerdeError<R, Q>
where
    Q: Copy + fmt::Debug,
{
    #[inline]
    fn from(error: MeasuredBudgetError<R, Q>) -> Self {
        match error {
            MeasuredBudgetError::Quantity { resource, source } => {
                Self::Quantity { resource, source }
            }
            MeasuredBudgetError::Budget(error) => Self::Budget(error),
        }
    }
}
