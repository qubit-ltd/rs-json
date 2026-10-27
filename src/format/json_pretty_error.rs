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
pub struct JsonPrettyError {
    kind: JsonPrettyErrorKind,
    offset: Option<usize>,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl JsonPrettyError {
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

    /// Returns the source byte offset when the error has one.
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
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|error| error as _)
    }
}
