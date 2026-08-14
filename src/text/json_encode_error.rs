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
use serde_json::Error as JsonError;
use thiserror::Error;

use crate::budget::JsonSyntaxError;

/// Failure produced while encoding one JSON document.
#[must_use]
#[derive(Debug, Error)]
pub enum JsonEncodeError<R, Q = usize>
where
    Q: Copy + Debug,
{
    /// Resource accounting rejected the value or output.
    #[error(transparent)]
    Budget(#[from] MeasuredBudgetError<R, Q>),
    /// A `RawValue` field did not contain valid JSON text.
    #[error("JSON raw value is invalid: {0}")]
    InvalidRawJson(#[source] JsonSyntaxError),
    /// Serde could not serialize the source value.
    #[error("JSON serialization failed: {0}")]
    Serialize(#[source] JsonError),
    /// The final destination writer rejected buffered bytes.
    #[error("JSON output writer failed: {0}")]
    Write(#[source] IoError),
}
