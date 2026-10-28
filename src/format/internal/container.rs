// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::ContainerKind;

/// Tracks formatting state for one open container.
pub(in crate::format) struct Container {
    /// Container syntax used when preparing children and closing the frame.
    pub(in crate::format) kind: ContainerKind,
    /// Number of direct array elements or object entries already planned.
    pub(in crate::format) count: usize,
    /// Whether an object key has been seen and its value is still pending.
    pub(in crate::format) awaiting_value: bool,
}
