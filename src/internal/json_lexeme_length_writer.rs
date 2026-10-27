// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Counts compact JSON scalar bytes without retaining their contents.

use std::io;
use std::io::Write;

/// An infallible output-length counter for serde_json formatters.
pub(crate) struct JsonLexemeLengthWriter {
    /// Total number of bytes accepted by the writer.
    len: usize,
}

impl JsonLexemeLengthWriter {
    /// Creates an empty byte counter.
    #[must_use]
    #[inline(always)]
    pub(crate) const fn new() -> Self {
        Self { len: 0 }
    }

    /// Returns the number of bytes accepted so far.
    #[must_use]
    #[inline(always)]
    pub(crate) const fn len(&self) -> usize {
        self.len
    }
}

impl Write for JsonLexemeLengthWriter {
    /// Adds the complete input length to the counter.
    ///
    /// # Returns
    ///
    /// The number of bytes accepted, which is always `input.len()` on success.
    ///
    /// # Errors
    ///
    /// Returns an [`io::ErrorKind::FileTooLarge`] error if the accumulated
    /// length cannot be represented as a `usize`.
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.len = self
            .len
            .checked_add(input.len())
            .ok_or_else(|| io::Error::from(io::ErrorKind::FileTooLarge))?;
        Ok(input.len())
    }

    /// Completes a no-op flush because no bytes are retained.
    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;
    use std::io::Write;

    use super::JsonLexemeLengthWriter;

    #[test]
    fn test_overflow_preserves_count_and_flush_is_a_noop() {
        let mut writer = JsonLexemeLengthWriter { len: usize::MAX - 1 };
        assert_eq!(writer.write(b"a").expect("last byte fits"), 1);
        assert_eq!(
            writer.write(b"b").expect_err("count must not wrap").kind(),
            ErrorKind::FileTooLarge
        );
        assert_eq!(writer.len(), usize::MAX);
        assert_eq!(writer.write(b"").expect("empty write still fits"), 0);
        writer.flush().expect("counter has no buffered output");
        assert_eq!(writer.len(), usize::MAX);
    }
}
