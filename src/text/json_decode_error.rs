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
use serde_json::error::Category;
use thiserror::Error;

use super::JsonSyntaxError;
use crate::internal::JsonLexicalError;

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
    #[error(
        "JSON deserialization failed ({category:?}) at line {line}, column {column}"
    )]
    Deserialize {
        /// Broad Serde failure category.
        category: Category,
        /// One-based line reported by Serde, or zero when unavailable.
        line: usize,
        /// One-based column reported by Serde, or zero when unavailable.
        column: usize,
    },
}

impl<R, Q> JsonDecodeError<R, Q>
where
    Q: Copy + Debug,
{
    /// Converts a shared lexical failure at the strict text boundary.
    #[inline]
    pub(crate) fn from_lexical(error: JsonLexicalError<R, Q>) -> Self {
        match error {
            JsonLexicalError::Budget(error) => Self::Budget(error),
            JsonLexicalError::Syntax(failure) => {
                Self::Syntax(JsonSyntaxError::from_lexical(failure))
            }
        }
    }

    /// Copies privacy-safe metadata from a Serde JSON error.
    #[inline]
    pub(super) fn from_serde(error: &JsonError) -> Self {
        Self::Deserialize {
            category: error.classify(),
            line: error.line(),
            column: error.column(),
        }
    }
}
