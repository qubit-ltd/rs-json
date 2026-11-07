// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Retains source errors for JCS encoding failures.

use std::fmt;
use std::io;

use qubit_budget::MeasuredBudgetError;

use crate::encode::JcsWorkingMemoryError;
use crate::encode::JsonSerializationError;

/// Internal source retained for each public JCS encoding failure category.
///
/// # Type Parameters
///
/// * `R` is the resource type used by the encoder's budget accounting.
/// * `Q` is the quantity type used by that accounting.
#[derive(Debug)]
pub(in crate::encode) enum JcsEncodeFailure<R, Q>
where
    Q: Copy + fmt::Debug,
{
    /// Resource accounting rejected the value or output.
    Budget(
        /// The measured resource or quantity failure reported by budget
        /// accounting.
        MeasuredBudgetError<R, Q>,
    ),
    /// Rendering exceeded its auxiliary-memory limit.
    WorkingMemory(
        /// The working-memory limit failure retained from rendering.
        JcsWorkingMemoryError,
    ),
    /// Canonical serialization could not represent the value.
    Serialize(
        /// The privacy-safe serialization failure retained from
        /// canonicalization.
        JsonSerializationError,
    ),
    /// The destination rejected output bytes.
    Write(
        /// The I/O failure returned while writing the encoded bytes.
        io::Error,
    ),
}
