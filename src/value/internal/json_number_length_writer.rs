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
    /// Accumulated UTF-8 byte count, saturating at `usize::MAX`.
    len: usize,
}

impl JsonNumberLengthWriter {
    /// Creates an empty counter.
    ///
    /// # Returns
    ///
    /// A counter initialized with a byte count of zero.
    #[must_use]
    #[inline]
    pub(crate) const fn new() -> Self {
        Self { len: 0 }
    }

    /// Returns the number of UTF-8 bytes written to this counter.
    ///
    /// The count saturates at `usize::MAX` if the total written length exceeds
    /// the largest representable `usize` value.
    ///
    /// # Returns
    ///
    /// The accumulated UTF-8 byte count, saturated at `usize::MAX`.
    #[must_use]
    #[inline]
    pub(crate) const fn len(&self) -> usize {
        self.len
    }
}

impl Write for JsonNumberLengthWriter {
    /// Adds the UTF-8 byte length of `text` to the counter without retaining
    /// it, saturating the count at `usize::MAX` if addition would overflow.
    ///
    /// # Parameters
    ///
    /// * `text` - Text whose UTF-8 byte length is added to the counter.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after updating the counter.
    ///
    /// # Errors
    ///
    /// This implementation does not return an error.
    #[inline]
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.len = self.len.saturating_add(text.len());
        Ok(())
    }
}
