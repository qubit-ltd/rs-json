// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Domain-independent failures produced by shared lexical scanning.

use std::fmt;

use qubit_budget::MeasuredBudgetError;

use super::json_lexical_failure::JsonLexicalFailure;

/// Failure produced while lexically scanning and charging one JSON document.
#[derive(Debug)]
pub(crate) enum JsonLexicalError<R, Q>
where
    Q: Copy + fmt::Debug,
{
    /// A decoded-value resource measurement was rejected.
    Budget(MeasuredBudgetError<R, Q>),
    /// The input is not one complete lexical JSON value.
    Syntax(JsonLexicalFailure),
}

impl<R, Q> From<MeasuredBudgetError<R, Q>> for JsonLexicalError<R, Q>
where
    Q: Copy + fmt::Debug,
{
    /// Preserves a measured budget rejection from lexical accounting.
    #[inline]
    fn from(error: MeasuredBudgetError<R, Q>) -> Self {
        Self::Budget(error)
    }
}
