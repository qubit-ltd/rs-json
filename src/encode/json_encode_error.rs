// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines errors returned by strict JSON encoding.

use std::fmt::Debug;
use std::io::Error as IoError;

use qubit_budget::MeasuredBudgetError;
use thiserror::Error;

use super::JsonSerializationError;
use crate::decode::JsonSyntaxError;

/// Failure produced while encoding one JSON document.
///
/// This enum intentionally remains exhaustive: its variants are a stable
/// compatibility contract for callers that match encode outcomes. Adding a
/// variant requires a breaking release rather than silently weakening those
/// matches with `#[non_exhaustive]`.
///
/// # Type Parameters
///
/// * `R` - Resource identity attached to budget failures.
/// * `Q` - Quantity representation attached to budget failures.
///
/// # Examples
///
/// ```
/// use qubit_budget::json::JsonResource;
/// use qubit_json::encode::JsonEncodeError;
/// use qubit_json::encode::JsonSerializationError;
/// use qubit_json::encode::JsonSerializationErrorKind;
///
/// let error: JsonEncodeError<JsonResource> = JsonEncodeError::Serialize(
///     JsonSerializationError::new(JsonSerializationErrorKind::CustomSerialization),
/// );
/// assert!(matches!(error, JsonEncodeError::Serialize(_)));
/// ```
#[must_use]
#[derive(Debug, Error)]
pub enum JsonEncodeError<R, Q = usize>
where
    Q: Copy + Debug,
{
    /// Resource accounting rejected the value or output.
    #[error(transparent)]
    Budget(
        /// Resource measurement that exceeded the configured limit.
        #[from]
        MeasuredBudgetError<R, Q>,
    ),
    /// A `RawValue` field did not contain valid JSON text.
    #[error("JSON raw value is invalid: {0}")]
    InvalidRawJson(
        /// Syntax failure from a `RawValue` payload.
        #[source]
        JsonSyntaxError,
    ),
    /// Serde could not serialize the source value.
    #[error("JSON serialization failed: {0}")]
    Serialize(
        /// Stable, privacy-safe reason serialization failed.
        #[source]
        JsonSerializationError,
    ),
    /// The external destination writer rejected output bytes.
    #[error("JSON output writer failed: {0}")]
    Write(
        /// I/O error raised while writing buffered or incremental output.
        #[source]
        IoError,
    ),
}
