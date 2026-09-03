// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the owned sources retained by strict JSON encoding errors.

use std::fmt;
use std::io::Error as IoError;

use qubit_budget::MeasuredBudgetError;

use super::JsonSerializationError;
use crate::decode::JsonSyntaxError;

/// An owned source extracted from a strict JSON encoding failure.
///
/// This enum lets callers move an underlying failure into another error model
/// with one exhaustive match. It avoids checking an error kind and then using
/// a separate optional extractor whose success depends on that prior check.
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
/// use qubit_json::encode::JsonEncodeErrorSource;
/// use qubit_json::encode::JsonEncoder;
///
/// let mut encoder = JsonEncoder::unlimited();
/// let error = encoder
///     .to_vec(&u128::MAX)
///     .expect_err("wide integer must not serialize as JSON");
/// match error.into_source() {
///     JsonEncodeErrorSource::Serialize(source) => {
///         assert!(source.is_number_error());
///     }
///     source => panic!("unexpected encoding source: {source:?}"),
/// }
/// # let _: Option<JsonEncodeErrorSource<JsonResource>> = None;
/// ```
#[must_use]
#[derive(Debug)]
pub enum JsonEncodeErrorSource<R, Q = usize>
where
    Q: Copy + fmt::Debug,
{
    /// Resource accounting rejected the encoded work or output.
    Budget(MeasuredBudgetError<R, Q>),
    /// A `serde_json::value::RawValue` payload was not strict JSON.
    InvalidRawJson(JsonSyntaxError),
    /// Serde rejected the source value during strict serialization.
    Serialize(JsonSerializationError),
    /// The destination writer rejected an output operation.
    Write(IoError),
}
