// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Failure reported while reserving tracked JCS working memory.

use crate::encode::JcsWorkingMemoryError;

/// Failure reported while reserving tracked working memory.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JcsReserveError {
    /// The requested capacity would exceed the configured working-memory limit.
    Limit(JcsWorkingMemoryError),
    /// The allocator could not reserve the requested capacity.
    Allocation,
}
