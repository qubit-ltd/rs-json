// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines errors returned by strict JSON encoding.

use std::error::Error;
use std::fmt;
use std::io::Error as IoError;

use qubit_budget::MeasuredBudgetError;

use super::JsonEncodeErrorKind;
use super::JsonSerializationError;
use super::internal::JsonEncodeFailure;
use crate::decode::JsonSyntaxError;

/// Failure produced while encoding one JSON document.
///
/// Internal failure variants remain private. Callers branch through
/// [`JsonEncodeErrorKind`] and inspect sources through stable accessors rather
/// than depending on the encoder's representation.
///
/// # Type Parameters
///
/// * `R` - Resource identity attached to budget failures.
/// * `Q` - Quantity representation attached to budget failures.
///
/// # Examples
///
/// ```
/// use qubit_json::encode::JsonEncodeErrorKind;
/// use qubit_json::encode::JsonEncoder;
///
/// let mut encoder = JsonEncoder::unlimited();
/// let error = encoder
///     .to_vec(&u128::MAX)
///     .expect_err("wide integer must not serialize as JSON");
/// assert_eq!(error.kind(), JsonEncodeErrorKind::Serialize);
/// assert!(error.serialization_error().is_some());
/// ```
#[must_use]
#[derive(Debug)]
pub struct JsonEncodeError<R, Q = usize>
where
    Q: Copy + fmt::Debug,
{
    /// Mutually exclusive structured failure retained by this error.
    failure: JsonEncodeFailure<R, Q>,
}

impl<R, Q> JsonEncodeError<R, Q>
where
    Q: Copy + fmt::Debug,
{
    /// Creates a resource-accounting failure.
    #[must_use = "return or inspect the constructed encoding error"]
    pub(crate) const fn budget(source: MeasuredBudgetError<R, Q>) -> Self {
        Self {
            failure: JsonEncodeFailure::Budget(source),
        }
    }

    /// Creates a failure for invalid `RawValue` JSON text.
    #[must_use = "return or inspect the constructed encoding error"]
    pub(crate) const fn invalid_raw_json(source: JsonSyntaxError) -> Self {
        Self {
            failure: JsonEncodeFailure::InvalidRawJson(source),
        }
    }

    /// Creates a privacy-safe Serde serialization failure.
    #[must_use = "return or inspect the constructed encoding error"]
    pub(crate) const fn serialization(source: JsonSerializationError) -> Self {
        Self {
            failure: JsonEncodeFailure::Serialize(source),
        }
    }

    /// Creates a destination-writer failure.
    #[must_use = "return or inspect the constructed encoding error"]
    pub(crate) fn write(source: IoError) -> Self {
        Self {
            failure: JsonEncodeFailure::Write(source),
        }
    }

    /// Returns the stable failure category.
    ///
    /// # Returns
    ///
    /// The category describing which encoding operation failed.
    #[must_use]
    #[inline(always)]
    pub const fn kind(&self) -> JsonEncodeErrorKind {
        match self.failure {
            JsonEncodeFailure::Budget(_) => JsonEncodeErrorKind::Budget,
            JsonEncodeFailure::InvalidRawJson(_) => JsonEncodeErrorKind::InvalidRawJson,
            JsonEncodeFailure::Serialize(_) => JsonEncodeErrorKind::Serialize,
            JsonEncodeFailure::Write(_) => JsonEncodeErrorKind::Write,
        }
    }

    /// Returns the measured budget failure when accounting rejected work.
    ///
    /// # Returns
    ///
    /// `Some` with the borrowed budget error for
    /// [`JsonEncodeErrorKind::Budget`], or `None` for every other failure
    /// kind.
    #[must_use]
    #[inline(always)]
    pub const fn budget_error(&self) -> Option<&MeasuredBudgetError<R, Q>> {
        match &self.failure {
            JsonEncodeFailure::Budget(source) => Some(source),
            _ => None,
        }
    }

    /// Returns the syntax error retained for invalid raw JSON.
    ///
    /// # Returns
    ///
    /// `Some` with the borrowed syntax error for
    /// [`JsonEncodeErrorKind::InvalidRawJson`], or `None` otherwise.
    #[must_use]
    #[inline(always)]
    pub const fn syntax_error(&self) -> Option<&JsonSyntaxError> {
        match &self.failure {
            JsonEncodeFailure::InvalidRawJson(source) => Some(source),
            _ => None,
        }
    }

    /// Returns the privacy-safe Serde serialization error when present.
    ///
    /// # Returns
    ///
    /// `Some` with the borrowed serialization error for
    /// [`JsonEncodeErrorKind::Serialize`], or `None` otherwise.
    #[must_use]
    #[inline(always)]
    pub const fn serialization_error(&self) -> Option<&JsonSerializationError> {
        match &self.failure {
            JsonEncodeFailure::Serialize(source) => Some(source),
            _ => None,
        }
    }

    /// Returns the destination-writer error when present.
    ///
    /// # Returns
    ///
    /// `Some` with the borrowed I/O error for [`JsonEncodeErrorKind::Write`],
    /// or `None` otherwise.
    #[must_use]
    #[inline(always)]
    pub const fn write_error(&self) -> Option<&IoError> {
        match &self.failure {
            JsonEncodeFailure::Write(source) => Some(source),
            _ => None,
        }
    }

    /// Consumes this error and returns its measured budget source when present.
    ///
    /// # Returns
    ///
    /// `Some` with the owned budget error for [`JsonEncodeErrorKind::Budget`],
    /// or `None` otherwise.
    #[must_use]
    pub fn into_budget_error(self) -> Option<MeasuredBudgetError<R, Q>> {
        match self.failure {
            JsonEncodeFailure::Budget(source) => Some(source),
            _ => None,
        }
    }

    /// Consumes this error and returns its syntax source when present.
    ///
    /// # Returns
    ///
    /// `Some` with the owned syntax error for
    /// [`JsonEncodeErrorKind::InvalidRawJson`], or `None` otherwise.
    #[must_use]
    pub fn into_syntax_error(self) -> Option<JsonSyntaxError> {
        match self.failure {
            JsonEncodeFailure::InvalidRawJson(source) => Some(source),
            _ => None,
        }
    }

    /// Consumes this error and returns its serialization source when present.
    ///
    /// # Returns
    ///
    /// `Some` with the owned serialization error for
    /// [`JsonEncodeErrorKind::Serialize`], or `None` otherwise.
    #[must_use]
    pub fn into_serialization_error(self) -> Option<JsonSerializationError> {
        match self.failure {
            JsonEncodeFailure::Serialize(source) => Some(source),
            _ => None,
        }
    }

    /// Consumes this error and returns its destination-writer source when
    /// present.
    ///
    /// # Returns
    ///
    /// `Some` with the owned I/O error for [`JsonEncodeErrorKind::Write`], or
    /// `None` otherwise.
    #[must_use]
    pub fn into_write_error(self) -> Option<IoError> {
        match self.failure {
            JsonEncodeFailure::Write(source) => Some(source),
            _ => None,
        }
    }
}

impl<R, Q> From<MeasuredBudgetError<R, Q>> for JsonEncodeError<R, Q>
where
    Q: Copy + fmt::Debug,
{
    /// Converts a measured-budget failure into an encoding failure.
    #[inline(always)]
    fn from(source: MeasuredBudgetError<R, Q>) -> Self {
        Self::budget(source)
    }
}

impl<R, Q> fmt::Display for JsonEncodeError<R, Q>
where
    R: fmt::Debug,
    Q: Copy + fmt::Debug + fmt::Display,
{
    /// Formats the retained source without exposing encoder internals.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.failure {
            JsonEncodeFailure::Budget(source) => fmt::Display::fmt(source, formatter),
            JsonEncodeFailure::InvalidRawJson(source) => write!(formatter, "JSON raw value is invalid: {source}"),
            JsonEncodeFailure::Serialize(source) => write!(formatter, "JSON serialization failed: {source}"),
            JsonEncodeFailure::Write(source) => write!(formatter, "JSON output writer failed: {source}"),
        }
    }
}

impl<R, Q> Error for JsonEncodeError<R, Q>
where
    R: fmt::Debug + 'static,
    Q: Copy + fmt::Debug + fmt::Display + 'static,
{
    /// Returns the structured budget, syntax, serialization, or I/O source.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.failure {
            JsonEncodeFailure::Budget(source) => Some(source),
            JsonEncodeFailure::InvalidRawJson(source) => Some(source),
            JsonEncodeFailure::Serialize(source) => Some(source),
            JsonEncodeFailure::Write(source) => Some(source),
        }
    }
}
