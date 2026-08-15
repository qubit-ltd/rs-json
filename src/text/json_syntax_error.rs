// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Structured location-aware JSON lexical errors.

use std::fmt;

use super::JsonSyntaxErrorReason;
use crate::internal::JsonLexicalFailure;

/// A JSON syntax error with byte and human-readable source coordinates.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JsonSyntaxError {
    /// Zero-based byte offset at which the error was observed.
    offset: usize,
    /// One-based source line containing the error.
    line: usize,
    /// One-based UTF-8 character column containing the error.
    column: usize,
    /// Stable classification of the lexical failure.
    reason: JsonSyntaxErrorReason,
}

impl JsonSyntaxError {
    /// Creates a syntax error from a source position and stable reason.
    #[inline]
    pub const fn new(
        offset: usize,
        line: usize,
        column: usize,
        reason: JsonSyntaxErrorReason,
    ) -> Self {
        Self {
            offset,
            line,
            column,
            reason,
        }
    }

    /// Converts one crate-private lexical failure at the text-domain boundary.
    pub(crate) fn from_lexical(failure: JsonLexicalFailure) -> Self {
        Self::new(
            failure.offset,
            failure.line,
            failure.column,
            failure.reason.into(),
        )
    }

    /// Returns the zero-based byte offset.
    #[must_use]
    #[inline(always)]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the one-based source line.
    #[must_use]
    #[inline(always)]
    pub const fn line(&self) -> usize {
        self.line
    }

    /// Returns the one-based UTF-8 character column.
    #[must_use]
    #[inline(always)]
    pub const fn column(&self) -> usize {
        self.column
    }

    /// Returns the stable syntax classification.
    #[inline(always)]
    pub const fn reason(&self) -> JsonSyntaxErrorReason {
        self.reason
    }
}

impl fmt::Display for JsonSyntaxError {
    /// Formats the reason and complete source location.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at line {} column {} (byte offset {})",
            self.reason, self.line, self.column, self.offset,
        )
    }
}

impl std::error::Error for JsonSyntaxError {}
