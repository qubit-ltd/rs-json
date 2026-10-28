// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Structured location-aware JSON lexical errors.

use std::error::Error;
use std::fmt;

use super::JsonDecodeContext;
use super::JsonSyntaxErrorReason;
use crate::lexical::JsonLexicalFailure;

/// A JSON syntax error with a zero-based byte offset and one-based source
/// coordinates. Its column counts UTF-8 characters; CRLF is one line break,
/// while LF and standalone CR each also start a new line.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::{JsonSyntaxError, JsonSyntaxErrorReason};
///
/// let error = JsonSyntaxError::new(
///     0,
///     1,
///     1,
///     JsonSyntaxErrorReason::UnexpectedEnd,
/// );
/// assert_eq!(error.line(), 1);
/// assert_eq!(error.reason(), JsonSyntaxErrorReason::UnexpectedEnd);
/// ```
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
    /// Grammar location where the lexical failure occurred, when known.
    context: Option<JsonDecodeContext>,
}

impl JsonSyntaxError {
    /// Creates a syntax error from a source position and stable reason.
    ///
    /// # Parameters
    ///
    /// * `offset` - Zero-based byte offset at which the error was observed.
    /// * `line` - One-based source line containing the error.
    /// * `column` - One-based UTF-8 character column containing the error.
    /// * `reason` - Stable classification of the lexical failure.
    ///
    /// # Returns
    ///
    /// A syntax error containing the supplied location and classification.
    #[inline]
    pub const fn new(offset: usize, line: usize, column: usize, reason: JsonSyntaxErrorReason) -> Self {
        Self {
            offset,
            line,
            column,
            reason,
            context: None,
        }
    }

    /// Converts a crate-private lexical failure into the text-domain syntax
    /// error.
    ///
    /// The byte offset, line, and column are preserved, while the lexical
    /// reason is mapped to its syntax-error classification.
    ///
    /// # Parameters
    ///
    /// * `failure` - The lexical failure whose location and reason are
    ///   converted.
    ///
    /// # Returns
    ///
    /// A syntax error with the lexical failure's source coordinates and mapped
    /// reason.
    pub(crate) fn from_lexical(failure: JsonLexicalFailure) -> Self {
        let mut error = Self::new(failure.offset, failure.line, failure.column, failure.reason.into());
        error.context = failure.context;
        error
    }

    /// Returns the grammar location where the error occurred, when known.
    #[must_use]
    #[inline(always)]
    pub const fn context(&self) -> Option<JsonDecodeContext> {
        self.context
    }

    /// Returns this error with the supplied grammar context.
    #[must_use = "the updated syntax error must be used"]
    #[inline(always)]
    pub const fn with_context(mut self, context: JsonDecodeContext) -> Self {
        self.context = Some(context);
        self
    }

    /// Returns the zero-based byte offset.
    ///
    /// # Returns
    ///
    /// The zero-based byte offset at which the error was observed.
    #[must_use]
    #[inline(always)]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the one-based source line.
    ///
    /// # Returns
    ///
    /// The one-based source line containing the error.
    #[must_use]
    #[inline(always)]
    pub const fn line(&self) -> usize {
        self.line
    }

    /// Returns the one-based UTF-8 character column.
    ///
    /// # Returns
    ///
    /// The one-based UTF-8 character column containing the error.
    #[must_use]
    #[inline(always)]
    pub const fn column(&self) -> usize {
        self.column
    }

    /// Returns the stable syntax classification.
    ///
    /// # Returns
    ///
    /// The stable lexical failure classification.
    #[inline(always)]
    pub const fn reason(&self) -> JsonSyntaxErrorReason {
        self.reason
    }
}

impl fmt::Display for JsonSyntaxError {
    /// Formats the reason and complete source location for human-readable
    /// diagnostics.
    ///
    /// # Errors
    ///
    /// Returns the formatter's error if writing the diagnostic to its output
    /// fails.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at line {} column {} (byte offset {})",
            self.reason, self.line, self.column, self.offset,
        )
    }
}

impl Error for JsonSyntaxError {}
