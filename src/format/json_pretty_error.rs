// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Errors returned by JSON token formatting.

use std::error::Error;
use std::fmt;

use super::JsonPrettyErrorKind;

/// Error from validating, planning, or writing formatted JSON tokens.
///
/// Syntax errors can carry a byte offset into the input, while output limits,
/// capacity failures, and I/O errors do not. A source error is available only
/// when the failure has an underlying cause.
///
/// ```
/// use std::error::Error;
///
/// use qubit_json::format::JsonPrettyErrorKind;
/// use qubit_json::format::JsonPrettyOptions;
/// use qubit_json::format::format_json_tokens;
///
/// let options = JsonPrettyOptions::builder().build()?;
/// let syntax = format_json_tokens(b"{}x", &options, None).expect_err("trailing data is invalid");
/// assert_eq!(syntax.kind(), JsonPrettyErrorKind::Syntax);
/// assert_eq!(syntax.offset(), Some(2));
/// assert!(syntax.source().is_some());
///
/// let limit = format_json_tokens(b"0", &options, Some(0)).expect_err("zero output allowance");
/// assert_eq!(limit.kind(), JsonPrettyErrorKind::OutputLimit);
/// assert_eq!(limit.offset(), None);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct JsonPrettyError {
    /// Stable category used to distinguish formatting failure classes.
    kind: JsonPrettyErrorKind,
    /// Available zero-based byte position in the source input, when applicable.
    offset: Option<usize>,
    /// Optional underlying cause, exposed separately through [`Error::source`].
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl JsonPrettyError {
    /// Creates an error with its category, optional source position, and cause.
    ///
    /// `offset` is a zero-based byte coordinate in the input, not a character
    /// index. `source` may be absent, including for errors whose category is
    /// [`JsonPrettyErrorKind::Syntax`].
    ///
    /// # Parameters
    ///
    /// * `kind` — stable category of the formatting failure.
    /// * `offset` — source input byte coordinate when one is available.
    /// * `source` — optional underlying error providing additional cause
    ///   context.
    pub(super) fn new(
        kind: JsonPrettyErrorKind,
        offset: Option<usize>,
        source: Option<Box<dyn Error + Send + Sync>>,
    ) -> Self {
        Self { kind, offset, source }
    }

    /// Returns the stable error category.
    pub fn kind(&self) -> JsonPrettyErrorKind {
        self.kind
    }

    /// Returns an available zero-based byte coordinate in the source input.
    ///
    /// `None` means the error is not associated with a source position; it does
    /// not mean byte zero. Not every syntax error is guaranteed to have an
    /// offset.
    ///
    /// # Returns
    ///
    /// The source input byte offset, or `None` when no coordinate is available.
    pub fn offset(&self) -> Option<usize> {
        self.offset
    }
}

impl fmt::Debug for JsonPrettyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JsonPrettyError")
            .field("kind", &self.kind)
            .field("offset", &self.offset)
            .finish()
    }
}

impl fmt::Display for JsonPrettyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.offset {
            Some(offset) => write!(formatter, "{} at byte offset {offset}", self.kind),
            None => fmt::Display::fmt(&self.kind, formatter),
        }
    }
}

impl Error for JsonPrettyError {
    /// Returns the optional underlying parsing, allocation, or I/O cause.
    ///
    /// Formatting and debug output do not include this cause automatically.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|error| error as _)
    }
}
