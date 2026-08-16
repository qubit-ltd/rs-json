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
use qubit_budget::ResourceQuantity;
use serde_json::Error as JsonError;
use thiserror::Error;

use crate::decode::JsonSyntaxError;

/// Failure produced while encoding one JSON document.
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

impl<R, Q> serde::ser::Error for JsonEncodeError<R, Q>
where
    R: Debug,
    Q: ResourceQuantity,
{
    /// Converts a custom Serde failure into the encode-specific error.
    fn custom<T>(message: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::Serialize(<JsonError as serde::ser::Error>::custom(message))
    }
}
