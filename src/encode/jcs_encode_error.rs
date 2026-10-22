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

#[derive(Debug)]
enum JcsEncodeFailure<R, Q>
where
    Q: Copy + fmt::Debug,
{
    Budget(MeasuredBudgetError<R, Q>),
    WorkingMemory(JcsWorkingMemoryError),
    Serialize(JsonSerializationError),
    Write(io::Error),
}

/// Failure produced while encoding a JSON value as RFC 8785 JCS.
///
/// Serialization diagnostics are deliberately fixed text and never expose
/// details returned by the canonicalizer.
#[must_use]
#[derive(Debug)]
pub struct JcsEncodeError<R, Q = usize>
where
    Q: Copy + fmt::Debug,
{
    failure: JcsEncodeFailure<R, Q>,
}

impl<R, Q> JcsEncodeError<R, Q>
where
    Q: Copy + fmt::Debug,
{
    pub(crate) const fn budget(source: MeasuredBudgetError<R, Q>) -> Self {
        Self {
            failure: JcsEncodeFailure::Budget(source),
        }
    }

    pub(crate) const fn serialization(source: JsonSerializationError) -> Self {
        Self {
            failure: JcsEncodeFailure::Serialize(source),
        }
    }

    pub(crate) const fn working_memory(source: JcsWorkingMemoryError) -> Self {
        Self {
            failure: JcsEncodeFailure::WorkingMemory(source),
        }
    }

    pub(crate) fn write(source: io::Error) -> Self {
        Self {
            failure: JcsEncodeFailure::Write(source),
        }
    }

    /// Converts a strict JSON encoder failure to its corresponding JCS class.
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
    #[must_use]
    pub const fn kind(&self) -> JcsEncodeErrorKind {
        match self.failure {
            JcsEncodeFailure::Budget(_) => JcsEncodeErrorKind::Budget,
            JcsEncodeFailure::WorkingMemory(_) => JcsEncodeErrorKind::WorkingMemory,
            JcsEncodeFailure::Serialize(_) => JcsEncodeErrorKind::Serialize,
            JcsEncodeFailure::Write(_) => JcsEncodeErrorKind::Write,
        }
    }

    /// Returns the measured budget failure, when resource accounting failed.
    #[must_use]
    pub const fn budget_error(&self) -> Option<&MeasuredBudgetError<R, Q>> {
        match &self.failure {
            JcsEncodeFailure::Budget(source) => Some(source),
            _ => None,
        }
    }

    /// Returns the working memory limit failure, when present.
    #[must_use]
    pub const fn working_memory_error(&self) -> Option<&JcsWorkingMemoryError> {
        match &self.failure {
            JcsEncodeFailure::WorkingMemory(source) => Some(source),
            _ => None,
        }
    }

    /// Returns the privacy-safe serialization failure, when present.
    #[must_use]
    pub const fn serialization_error(&self) -> Option<&JsonSerializationError> {
        match &self.failure {
            JcsEncodeFailure::Serialize(source) => Some(source),
            _ => None,
        }
    }

    /// Returns the destination I/O failure, when present.
    #[must_use]
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
    fn from(source: MeasuredBudgetError<R, Q>) -> Self {
        Self::budget(source)
    }
}
