// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Categories of JSON token formatting failures.

use std::fmt;

/// Stable category for a token formatting failure.
///
/// The category distinguishes invalid input, output limits, capacity failures,
/// and write failures without exposing implementation-specific details.
///
/// # Examples
///
/// ```
/// use qubit_json::format::JsonPrettyErrorKind;
///
/// let kind = JsonPrettyErrorKind::Syntax;
/// assert_eq!(kind.to_string(), "invalid JSON syntax");
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonPrettyErrorKind {
    /// The input is not one complete syntactically valid JSON value.
    Syntax,
    /// The formatted bytes exceed the caller-provided maximum.
    OutputLimit,
    /// Checked length arithmetic or allocation could not be completed.
    Capacity,
    /// The output rejected a write; an accepted prefix may remain.
    Io,
}

impl fmt::Display for JsonPrettyErrorKind {
    /// Writes the stable human-readable description of this failure category.
    ///
    /// # Parameters
    ///
    /// * `formatter` — the output formatter receiving the description.
    ///   Formatting stops if it reports a write error.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after writing the description, or the formatter's
    /// error if the output sink rejects the write.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Syntax => "invalid JSON syntax",
            Self::OutputLimit => "formatted output limit exceeded",
            Self::Capacity => "formatting capacity exceeded",
            Self::Io => "formatted output write failed",
        })
    }
}
