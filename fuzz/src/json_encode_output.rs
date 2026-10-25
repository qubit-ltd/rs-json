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

/// Controls whether a writer completes, interrupts once, or fails after a
/// prefix.
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
    /// Creates a writer with an optional byte limit and per-call short-write
    /// cap.
    #[must_use]
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
    #[must_use]
    #[inline]
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

    #[inline]
    fn flush(&mut self) -> Result<(), Error> {
        Ok(())
    }
}
