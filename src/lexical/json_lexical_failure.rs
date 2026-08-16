// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Source coordinates and reason for a lexical JSON rejection.

use std::fmt;

use super::json_lexical_error_reason::JsonLexicalErrorReason;

/// Location and stable reason for a lexical JSON rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JsonLexicalFailure {
    /// Zero-based input byte offset.
    pub(crate) offset: usize,
    /// One-based input line.
    pub(crate) line: usize,
    /// One-based UTF-8 character column.
    pub(crate) column: usize,
    /// Stable lexical rejection reason.
    pub(crate) reason: JsonLexicalErrorReason,
}

impl fmt::Display for JsonLexicalFailure {
    /// Formats a privacy-safe lexical diagnostic and its source coordinates.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at line {} column {} (byte offset {})",
            self.reason, self.line, self.column, self.offset,
        )
    }
}

impl std::error::Error for JsonLexicalFailure {}
