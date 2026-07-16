// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the private representation of a Markdown opening fence.

/// Describes one recognized Markdown code-fence opening line.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MarkdownFence {
    /// Stores the byte marker used by the fence.
    pub(crate) marker: u8,
    /// Stores the number of repeated marker bytes in the opening fence.
    pub(crate) marker_len: usize,
    /// Stores the byte index immediately after the opening marker run.
    pub(crate) marker_end: usize,
}
