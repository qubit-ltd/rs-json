// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Retains safe metadata for strict JSON deserialization failures.

use serde_json::Error as JsonError;
use serde_json::error::Category;
use thiserror::Error;

use super::JsonDeserializeErrorCategory;

/// Safe metadata for one strict Serde failure.
///
/// The error preserves the broad category and source coordinates reported by
/// Serde without retaining an input-derived diagnostic message.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error(
    "JSON deserialization failed ({category}) at line {line}, column {column}"
)]
pub struct JsonDeserializeError {
    /// Broad Serde failure category.
    category: JsonDeserializeErrorCategory,

    /// One-based line reported by Serde, or zero when unavailable.
    line: usize,

    /// One-based column reported by Serde, or zero when unavailable.
    column: usize,
}

impl JsonDeserializeError {
    /// Converts Serde metadata without retaining its source message.
    ///
    /// # Parameters
    ///
    /// * `error` - Serde JSON error whose safe metadata is copied.
    ///
    /// # Returns
    ///
    /// A strict deserialization error containing only category and coordinates.
    #[inline]
    pub(crate) fn from_serde(error: &JsonError) -> Self {
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

    /// Safe metadata for an I/O failure with no source coordinates.
    pub(crate) const IO: Self = Self {
        category: JsonDeserializeErrorCategory::Io,
        line: 0,
        column: 0,
    };

    /// Returns the broad Serde failure category.
    ///
    /// # Returns
    ///
    /// The stable category copied from Serde.
    #[inline(always)]
    pub const fn category(&self) -> JsonDeserializeErrorCategory {
        self.category
    }

    /// Returns Serde's reported source line.
    ///
    /// # Returns
    ///
    /// The one-based line, or zero when no source position was available.
    #[must_use]
    #[inline(always)]
    pub const fn line(&self) -> usize {
        self.line
    }

    /// Returns Serde's reported source column.
    ///
    /// # Returns
    ///
    /// The one-based column, or zero when no source position was available.
    #[must_use]
    #[inline(always)]
    pub const fn column(&self) -> usize {
        self.column
    }
}

impl From<JsonError> for JsonDeserializeError {
    /// Copies safe category and coordinate metadata from a Serde JSON error.
    #[inline]
    fn from(error: JsonError) -> Self {
        Self::from_serde(&error)
    }
}
