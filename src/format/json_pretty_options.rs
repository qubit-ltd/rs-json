// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Options for whitespace-only JSON token formatting.

use super::JsonPrettyOptionsBuilder;

/// Controls indentation and the optional final newline in pretty output.
///
/// The default is two spaces per level, no displayed-depth cap, and no final
/// newline. A displayed-depth cap changes indentation only; it does not limit
/// the actual nesting depth accepted by the formatter.
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
pub struct JsonPrettyOptions {
    /// Spaces used per displayed indentation level.
    pub(super) indent_width: usize,
    /// Optional cap on displayed indentation depth.
    pub(super) max_indent_levels: Option<usize>,
    /// Whether to append one newline after the formatted value.
    pub(super) trailing_newline: bool,
}

impl JsonPrettyOptions {
    /// Creates a builder using two spaces, unlimited displayed depth, and no
    /// final newline.
    ///
    /// # Returns
    ///
    /// A builder initialized with the documented formatting defaults.
    pub fn builder() -> JsonPrettyOptionsBuilder {
        JsonPrettyOptionsBuilder::default()
    }
}
