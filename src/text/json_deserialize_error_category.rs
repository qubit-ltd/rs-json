// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Classifies safe errors returned by strict JSON deserialization.

use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

/// A broad strict Serde failure category that retains no input-derived text.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonDeserializeErrorCategory {
    /// The target type rejected an otherwise valid JSON value.
    Data,

    /// The input ended before the target value was complete.
    Eof,

    /// Serde reported an I/O failure while reading the input.
    Io,

    /// The input failed Serde's syntax checks.
    Syntax,
}

impl Display for JsonDeserializeErrorCategory {
    /// Writes the stable lowercase name of this error category.
    ///
    /// # Parameters
    ///
    /// * `formatter` - Destination for the category name.
    ///
    /// # Returns
    ///
    /// The destination formatter result.
    ///
    /// # Errors
    ///
    /// Returns a formatting error when the destination rejects output.
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
