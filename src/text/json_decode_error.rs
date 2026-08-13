// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines errors returned by strict JSON decoding.

use std::fmt::Debug;

use qubit_budget::MeasuredBudgetError;
use serde_json::Error as JsonError;
use thiserror::Error;

use crate::budget::JsonSyntaxError;

/// Failure produced while decoding one strict JSON document.
#[must_use]
#[derive(Debug, Error)]
pub enum JsonDecodeError<R, Q = usize>
where
    Q: Copy + Debug,
{
    /// Resource accounting rejected the input or its decoded JSON value.
    #[error(transparent)]
    Budget(#[from] MeasuredBudgetError<R, Q>),
    /// The byte stream did not contain one complete JSON document.
    #[error(transparent)]
    Syntax(#[from] JsonSyntaxError),
    /// Serde rejected an otherwise admitted document for the requested type.
    #[error("JSON deserialization failed: {0}")]
    Deserialize(#[source] JsonError),
}
