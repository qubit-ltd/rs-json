// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::Action;

/// Completed formatting plan retained for rendering after validation succeeds.
pub(in crate::format) struct Plan {
    /// Prevalidated operations; rendering must wait until the complete plan
    /// succeeds.
    pub(in crate::format) actions: Vec<Action>,
    /// Full planned output size, including a trailing newline, even if a limit
    /// was exceeded.
    pub(in crate::format) length: usize,
}
