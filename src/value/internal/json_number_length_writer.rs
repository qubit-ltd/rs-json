// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Counts formatted Number bytes without retaining the representation.

use std::fmt;
use std::fmt::Write;

/// Infallible byte counter used by Number's Display implementation.
pub(crate) struct JsonNumberLengthWriter {
    len: usize,
}

impl JsonNumberLengthWriter {
    /// Creates an empty counter.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self { len: 0 }
    }

    /// Returns the accumulated byte count.
    #[must_use]
    pub(crate) const fn len(&self) -> usize {
        self.len
    }
}

impl Write for JsonNumberLengthWriter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.len = self.len.saturating_add(text.len());
        Ok(())
    }
}
