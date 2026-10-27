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
///
/// The displayed indentation cap does not constrain JSON nesting. All current
/// settings are accepted by [`Self::build`].
///
/// # Examples
///
/// ```
/// use qubit_json::format::{format_json_tokens, JsonPrettyOptions};
///
/// let options = JsonPrettyOptions::builder()
///     .indent_width(2)
///     .trailing_newline(true)
///     .build()?;
/// let output = format_json_tokens(b"[1]", &options, None)?;
/// assert_eq!(output, b"[\n  1\n]\n");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonPrettyOptionsBuilder {
    /// Spaces used per displayed indentation level.
    indent_width: usize,
    /// Optional cap on displayed indentation depth.
    max_indent_levels: Option<usize>,
    /// Whether to append a final newline.
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
    ///
    /// # Parameters
    ///
    /// * `spaces` - Number of spaces used for each displayed level.
    ///
    /// # Returns
    ///
    /// The builder with the requested indentation width.
    pub fn indent_width(mut self, spaces: usize) -> Self {
        self.indent_width = spaces;
        self
    }

    /// Caps displayed indentation while preserving real nesting state.
    ///
    /// `None` leaves displayed depth uncapped; `Some(0)` emits no indentation.
    /// The cap does not limit the input's actual nesting depth.
    ///
    /// # Parameters
    ///
    /// * `maximum` - Maximum displayed indentation level, if any.
    ///
    /// # Returns
    ///
    /// The builder with the displayed-depth cap configured.
    pub fn max_indent_levels(mut self, maximum: Option<usize>) -> Self {
        self.max_indent_levels = maximum;
        self
    }

    /// Controls whether one final newline is emitted.
    ///
    /// # Parameters
    ///
    /// * `enabled` - Whether to append one newline after the formatted value.
    ///
    /// # Returns
    ///
    /// The builder with the final-newline choice configured.
    pub fn trailing_newline(mut self, enabled: bool) -> Self {
        self.trailing_newline = enabled;
        self
    }

    /// Builds the options. All combinations are valid.
    ///
    /// The `Result` return type is retained for API compatibility; this method
    /// currently returns `Ok` for every configuration.
    ///
    /// # Returns
    ///
    /// The configured options. The result is currently always `Ok`.
    pub fn build(self) -> Result<JsonPrettyOptions, JsonPrettyError> {
        Ok(JsonPrettyOptions {
            indent_width: self.indent_width,
            max_indent_levels: self.max_indent_levels,
            trailing_newline: self.trailing_newline,
        })
    }
}
