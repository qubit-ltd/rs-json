// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Writer fixtures for checking incremental JSON output accounting.

use std::io::Error;
use std::io::ErrorKind;
use std::io::Write;

/// Controls whether a writer completes, interrupts once, or fails after a prefix.
#[derive(Clone, Copy)]
pub enum OutputMode {
    /// Accept every byte, optionally in short writes.
    Complete,
    /// Return `Interrupted` once, then accept every byte.
    InterruptOnce,
    /// Fail with `BrokenPipe` after accepting the configured prefix.
    BrokenPipe,
}

/// Records externally accepted bytes and can fail at a chosen boundary.
pub struct FuzzOutput {
    mode: OutputMode,
    remaining: usize,
    max_chunk: usize,
    interrupted: bool,
    bytes: Vec<u8>,
}

impl FuzzOutput {
    /// Creates a writer with an optional byte limit and per-call short-write cap.
    pub fn new(mode: OutputMode, remaining: usize, max_chunk: usize) -> Self {
        Self {
            mode,
            remaining,
            max_chunk: max_chunk.max(1),
            interrupted: false,
            bytes: Vec::new(),
        }
    }

    /// Returns the bytes accepted by the external writer.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Write for FuzzOutput {
    fn write(&mut self, buffer: &[u8]) -> Result<usize, Error> {
        if matches!(self.mode, OutputMode::InterruptOnce) && !self.interrupted {
            self.interrupted = true;
            return Err(Error::from(ErrorKind::Interrupted));
        }
        if matches!(self.mode, OutputMode::BrokenPipe) && self.remaining == 0 {
            return Err(Error::from(ErrorKind::BrokenPipe));
        }
        let count = buffer.len().min(self.max_chunk).min(self.remaining);
        self.bytes.extend_from_slice(&buffer[..count]);
        self.remaining -= count;
        Ok(count)
    }

    fn flush(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::FuzzOutput;
    use super::OutputMode;

    #[test]
    fn test_output_fixture_retries_interrupted_short_writes() {
        let mut output = FuzzOutput::new(OutputMode::InterruptOnce, usize::MAX, 2);
        output
            .write_all(b"record")
            .expect("interrupted short writer must complete");
        assert_eq!(output.bytes(), b"record");
    }

    #[test]
    fn test_output_fixture_fails_after_exact_prefix() {
        let mut output = FuzzOutput::new(OutputMode::BrokenPipe, 3, 2);
        let error = output
            .write_all(b"record")
            .expect_err("writer must fail after its configured prefix");
        assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
        assert_eq!(output.bytes(), b"rec");
    }
}
