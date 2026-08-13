// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines errors returned by strict JSON decoding.

use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use qubit_budget::MeasuredBudgetError;
use serde_json::error::Category;
use thiserror::Error;

use crate::budget::JsonSyntaxError;

/// Failure produced while decoding one strict JSON document.
#[must_use]
#[derive(Debug, Error)]
pub enum JsonDecodeError<R, Q: Copy + Debug = usize> {
    /// Resource accounting rejected the input or its decoded JSON value.
    #[error(transparent)]
    Budget(#[from] MeasuredBudgetError<R, Q>),
    /// The byte stream did not contain one complete JSON document.
    #[error(transparent)]
    Syntax(#[from] JsonSyntaxError),
    /// Serde rejected an otherwise admitted document for the requested type.
    #[error(transparent)]
    Deserialize(JsonDeserializeError),
}

/// Classifies a strict serde failure without retaining input-derived text.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonDeserializeErrorCategory {
    /// The target type rejected an otherwise valid JSON value.
    Data,
    /// The input ended before the target value was complete.
    Eof,
    /// Serde reported an I/O failure while reading the input.
    Io,
    /// The input failed serde's syntax checks.
    Syntax,
}

impl Display for JsonDeserializeErrorCategory {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        let name = match self {
            Self::Data => "data",
            Self::Eof => "eof",
            Self::Io => "io",
            Self::Syntax => "syntax",
        };
        formatter.write_str(name)
    }
}

/// Safe metadata for one strict serde failure.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error(
    "JSON deserialization failed ({category}) at line {line}, column {column}"
)]
pub struct JsonDeserializeError {
    /// Broad serde failure category.
    category: JsonDeserializeErrorCategory,
    /// One-based line reported by serde, or zero when unavailable.
    line: usize,
    /// One-based column reported by serde, or zero when unavailable.
    column: usize,
}

impl JsonDeserializeError {
    /// Converts serde metadata without retaining its source message.
    pub(crate) fn from_serde(error: &serde_json::Error) -> Self {
        Self {
            category: match error.classify() {
                Category::Data => JsonDeserializeErrorCategory::Data,
                Category::Eof => JsonDeserializeErrorCategory::Eof,
                Category::Io => JsonDeserializeErrorCategory::Io,
                Category::Syntax => JsonDeserializeErrorCategory::Syntax,
            },
            line: error.line(),
            column: error.column(),
        }
    }

    /// Creates safe metadata for an I/O failure without retaining the error.
    pub(crate) const fn io() -> Self {
        Self {
            category: JsonDeserializeErrorCategory::Io,
            line: 0,
            column: 0,
        }
    }

    /// Returns the broad serde failure category.
    pub const fn category(&self) -> JsonDeserializeErrorCategory {
        self.category
    }

    /// Returns serde's reported line, or zero when unavailable.
    #[must_use]
    pub const fn line(&self) -> usize {
        self.line
    }

    /// Returns serde's reported column, or zero when unavailable.
    #[must_use]
    pub const fn column(&self) -> usize {
        self.column
    }
}
