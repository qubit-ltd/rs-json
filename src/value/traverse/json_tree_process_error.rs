// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines failures produced while processing JSON trees.

use std::fmt::Debug;

use qubit_budget::MeasuredBudgetError;
use thiserror::Error;

/// Identifies whether JSON tree processing failed in infrastructure or domain
/// code.
///
/// This enum intentionally remains exhaustive so callers can distinguish the
/// complete set of processing failure domains at compile time. Adding a domain
/// requires a breaking release rather than a `#[non_exhaustive]` change.
///
/// # Type Parameters
///
/// * `R` - Resource identity attached to budget failures.
/// * `Q` - Quantity representation attached to budget failures.
/// * `E` - Error type returned by the visitor.
///
/// # Examples
///
/// ```
/// use qubit_json::value::traverse::JsonTreeProcessError;
///
/// let error: JsonTreeProcessError<(), usize, &str> =
///     JsonTreeProcessError::Visitor("visitor rejected the node");
/// assert!(matches!(error, JsonTreeProcessError::Visitor(_)));
/// ```
#[derive(Debug, Error)]
pub enum JsonTreeProcessError<R, Q, E>
where
    Q: Copy + Debug,
{
    /// A JSON resource measurement or budget rejected a tree node.
    #[error(transparent)]
    Budget(
        /// Resource measurement that exceeded the configured limit.
        #[from]
        MeasuredBudgetError<R, Q>,
    ),
    /// The caller-defined visitor rejected a budget-admitted node.
    #[error("JSON tree visitor failed")]
    Visitor(
        /// Domain error returned by the visitor callback.
        E,
    ),
}
