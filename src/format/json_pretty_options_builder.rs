// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Builder for JSON token formatting options.

use super::JsonPrettyError;
use super::JsonPrettyOptions;

/// Builder for whitespace formatting choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonPrettyOptionsBuilder {
    indent_width: usize,
    max_indent_levels: Option<usize>,
    trailing_newline: bool,
}

impl Default for JsonPrettyOptionsBuilder {
    fn default() -> Self {
        Self {
            indent_width: 2,
            max_indent_levels: None,
            trailing_newline: false,
        }
    }
}

impl JsonPrettyOptionsBuilder {
    /// Sets spaces per displayed indentation level; zero is valid.
    pub fn indent_width(mut self, spaces: usize) -> Self {
        self.indent_width = spaces;
        self
    }

    /// Caps displayed indentation while preserving real nesting state.
    pub fn max_indent_levels(mut self, maximum: Option<usize>) -> Self {
        self.max_indent_levels = maximum;
        self
    }

    /// Controls whether one final newline is emitted.
    pub fn trailing_newline(mut self, enabled: bool) -> Self {
        self.trailing_newline = enabled;
        self
    }

    /// Builds the options. All combinations are valid.
    pub fn build(self) -> Result<JsonPrettyOptions, JsonPrettyError> {
        Ok(JsonPrettyOptions {
            indent_width: self.indent_width,
            max_indent_levels: self.max_indent_levels,
            trailing_newline: self.trailing_newline,
        })
    }
}
