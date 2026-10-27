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
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Syntax => "invalid JSON syntax",
            Self::OutputLimit => "formatted output limit exceeded",
            Self::Capacity => "formatting capacity exceeded",
            Self::Io => "formatted output write failed",
        })
    }
}
