// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the private source representation behind JSON encoding errors.

use std::fmt::Debug;
use std::io::Error as IoError;

use qubit_budget::MeasuredBudgetError;

use crate::decode::JsonSyntaxError;
use crate::encode::JsonSerializationError;

/// Stores the mutually exclusive source retained by one encoding failure.
#[derive(Debug)]
pub(in crate::encode) enum JsonEncodeFailure<R, Q>
where
    Q: Copy + Debug,
{
    /// Resource accounting rejected a value or output measurement.
    Budget(MeasuredBudgetError<R, Q>),
    /// A RawValue payload failed strict JSON syntax validation.
    InvalidRawJson(JsonSyntaxError),
    /// Serde rejected a source value during strict serialization.
    Serialize(JsonSerializationError),
    /// The destination writer rejected one output operation.
    Write(IoError),
}
