// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines errors returned by JCS encoding.

use std::error::Error;
use std::fmt;
use std::io;

use qubit_budget::MeasuredBudgetError;

use super::JcsEncodeErrorKind;
use super::JcsWorkingMemoryError;
use super::JsonEncodeError;
use super::JsonEncodeErrorKind;
use super::JsonSerializationError;
use super::JsonSerializationErrorKind;
use super::internal::JcsEncodeFailure;

/// Failure produced while encoding a JSON value as RFC 8785 JCS.
///
/// Serialization diagnostics are deliberately fixed text and never expose
/// details returned by the canonicalizer.
///
/// # Type Parameters
///
/// * `R` is the resource type used by the encoder's budget accounting.
/// * `Q` is the quantity type used by that accounting.
///
/// # Examples
///
/// ```
/// use qubit_budget::json::JsonEncodeLimits;
/// use qubit_json::encode::JcsEncodeErrorKind;
/// use qubit_json::encode::JcsEncoder;
///
/// let limits: JsonEncodeLimits = JsonEncodeLimits::new();
/// let mut encoder = JcsEncoder::with_limits(limits, 0);
/// let error = encoder
///     .to_vec(&serde_json::json!({"key": "value"}))
///     .expect_err("zero working memory cannot render an object");
/// assert_eq!(error.kind(), JcsEncodeErrorKind::WorkingMemory);
/// ```
#[must_use]
#[derive(Debug)]
pub struct JcsEncodeError<R, Q = usize>
where
    Q: Copy + fmt::Debug,
{
    /// Retains the detailed source behind the stable public failure category.
    failure: JcsEncodeFailure<R, Q>,
}

impl<R, Q> JcsEncodeError<R, Q>
where
    Q: Copy + fmt::Debug,
{
    /// Wraps a budget-accounting failure without changing its source.
    ///
    /// # Parameters
    ///
    /// * `source` - Measured budget failure to retain.
    ///
    /// # Returns
    ///
    /// A JCS encoding error classified as a budget failure.
    #[inline]
    pub(crate) const fn budget(source: MeasuredBudgetError<R, Q>) -> Self {
        Self {
            failure: JcsEncodeFailure::Budget(source),
        }
    }

    /// Wraps a canonicalization failure while preserving its safe category.
    ///
    /// # Parameters
    ///
    /// * `source` - Privacy-safe serialization failure to retain.
    ///
    /// # Returns
    ///
    /// A JCS encoding error classified as a serialization failure.
    #[inline]
    pub(crate) const fn serialization(source: JsonSerializationError) -> Self {
        Self {
            failure: JcsEncodeFailure::Serialize(source),
        }
    }

    /// Wraps a working-memory limit failure.
    ///
    /// # Parameters
    ///
    /// * `source` - Working-memory limit failure to retain.
    ///
    /// # Returns
    ///
    /// A JCS encoding error classified as a working-memory failure.
    #[inline]
    pub(crate) const fn working_memory(source: JcsWorkingMemoryError) -> Self {
        Self {
            failure: JcsEncodeFailure::WorkingMemory(source),
        }
    }

    /// Wraps an output destination failure.
    ///
    /// # Parameters
    ///
    /// * `source` - I/O failure returned by the output destination.
    ///
    /// # Returns
    ///
    /// A JCS encoding error classified as a writer failure.
    #[inline]
    pub(crate) fn write(source: io::Error) -> Self {
        Self {
            failure: JcsEncodeFailure::Write(source),
        }
    }

    /// Converts a strict JSON encoder failure to its corresponding JCS class.
    ///
    /// # Parameters
    ///
    /// * `error` - Strict encoder failure to classify as a JCS error.
    ///
    /// # Returns
    ///
    /// The corresponding budget, serialization, or writer failure; invalid raw
    /// JSON becomes a privacy-safe serialization failure.
    ///
    /// # Panics
    ///
    /// Panics if the strict encoder reports a category without its required
    /// retained source, which violates the encoder error invariant.
    pub(crate) fn from_json_encode(error: JsonEncodeError<R, Q>) -> Self {
        match error.kind() {
            JsonEncodeErrorKind::Budget => {
                Self::budget(error.into_budget_error().expect("budget kind retains a budget source"))
            }
            JsonEncodeErrorKind::Serialize => Self::serialization(
                error
                    .into_serialization_error()
                    .expect("serialize kind retains a source"),
            ),
            JsonEncodeErrorKind::Write => {
                Self::write(error.into_write_error().expect("write kind retains an I/O source"))
            }
            JsonEncodeErrorKind::InvalidRawJson => Self::serialization(JsonSerializationError::new(
                JsonSerializationErrorKind::CustomSerialization,
            )),
        }
    }

    /// Returns the stable failure category.
    ///
    /// # Returns
    ///
    /// The category identifying the retained budget, memory, serialization, or
    /// writer failure.
    #[must_use]
    #[inline]
    pub const fn kind(&self) -> JcsEncodeErrorKind {
        match self.failure {
            JcsEncodeFailure::Budget(_) => JcsEncodeErrorKind::Budget,
            JcsEncodeFailure::WorkingMemory(_) => JcsEncodeErrorKind::WorkingMemory,
            JcsEncodeFailure::Serialize(_) => JcsEncodeErrorKind::Serialize,
            JcsEncodeFailure::Write(_) => JcsEncodeErrorKind::Write,
        }
    }

    /// Returns the measured budget failure when resource accounting failed.
    ///
    /// `Some` contains the retained accounting source; `None` means the error
    /// belongs to another category.
    ///
    /// # Returns
    ///
    /// The retained budget error for a budget failure, or `None` otherwise.
    #[inline]
    pub const fn budget_error(&self) -> Option<&MeasuredBudgetError<R, Q>> {
        match &self.failure {
            JcsEncodeFailure::Budget(source) => Some(source),
            _ => None,
        }
    }

    /// Returns the working-memory limit failure when present.
    ///
    /// `Some` contains the retained limit source; `None` means another failure
    /// occurred.
    ///
    /// # Returns
    ///
    /// The retained working-memory failure for that category, or `None`
    /// otherwise.
    #[inline]
    pub const fn working_memory_error(&self) -> Option<&JcsWorkingMemoryError> {
        match &self.failure {
            JcsEncodeFailure::WorkingMemory(source) => Some(source),
            _ => None,
        }
    }

    /// Returns the privacy-safe serialization failure when present.
    ///
    /// `Some` contains its stable category without canonicalizer details;
    /// `None` means another failure occurred.
    ///
    /// # Returns
    ///
    /// The privacy-safe serialization failure for that category, or `None`
    /// otherwise.
    #[inline]
    pub const fn serialization_error(&self) -> Option<&JsonSerializationError> {
        match &self.failure {
            JcsEncodeFailure::Serialize(source) => Some(source),
            _ => None,
        }
    }

    /// Returns the destination I/O failure when present.
    ///
    /// `Some` contains the writer's I/O source; `None` means another failure
    /// occurred.
    ///
    /// # Returns
    ///
    /// The retained writer error for that category, or `None` otherwise.
    #[inline]
    pub const fn write_error(&self) -> Option<&io::Error> {
        match &self.failure {
            JcsEncodeFailure::Write(source) => Some(source),
            _ => None,
        }
    }
}

impl<R, Q> fmt::Display for JcsEncodeError<R, Q>
where
    R: fmt::Debug,
    Q: Copy + fmt::Debug + fmt::Display,
{
    /// Formats a stable message that does not expose canonicalizer details.
    ///
    /// # Parameters
    ///
    /// * `formatter` - Destination that receives the error message.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the message is written.
    ///
    /// # Errors
    ///
    /// Returns the formatter error if writing the message fails.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.failure {
            JcsEncodeFailure::Budget(source) => fmt::Display::fmt(source, formatter),
            JcsEncodeFailure::WorkingMemory(source) => fmt::Display::fmt(source, formatter),
            JcsEncodeFailure::Serialize(_) => formatter.write_str("JCS serialization failed"),
            JcsEncodeFailure::Write(_) => formatter.write_str("JCS output writer failed"),
        }
    }
}

impl<R, Q> Error for JcsEncodeError<R, Q>
where
    R: fmt::Debug + 'static,
    Q: Copy + fmt::Debug + fmt::Display + 'static,
{
    /// Returns the retained source associated with the error category.
    ///
    /// # Returns
    ///
    /// The retained budget, working-memory, serialization, or writer source.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.failure {
            JcsEncodeFailure::Budget(source) => Some(source),
            JcsEncodeFailure::WorkingMemory(source) => Some(source),
            JcsEncodeFailure::Serialize(source) => Some(source),
            JcsEncodeFailure::Write(source) => Some(source),
        }
    }
}

impl<R, Q> From<MeasuredBudgetError<R, Q>> for JcsEncodeError<R, Q>
where
    Q: Copy + fmt::Debug,
{
    /// Converts a measured budget failure into a JCS encoding error.
    ///
    /// # Parameters
    ///
    /// * `source` - Budget failure to preserve.
    ///
    /// # Returns
    ///
    /// A JCS error classified as a budget failure.
    fn from(source: MeasuredBudgetError<R, Q>) -> Self {
        Self::budget(source)
    }
}
