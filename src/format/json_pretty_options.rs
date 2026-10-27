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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonPrettyOptions {
    pub(super) indent_width: usize,
    pub(super) max_indent_levels: Option<usize>,
    pub(super) trailing_newline: bool,
}

impl JsonPrettyOptions {
    /// Creates a builder using two spaces, unlimited displayed depth, and no
    /// final newline.
    pub fn builder() -> JsonPrettyOptionsBuilder {
        JsonPrettyOptionsBuilder::default()
    }
}
