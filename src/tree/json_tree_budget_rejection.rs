// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines mutable traversal behavior after a budget rejection.

/// Selects whether a rejected JSON node aborts processing or is safely skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonTreeBudgetRejection {
    /// Return the original budget error to the caller.
    Abort,
    /// Continue after the visitor has handled the rejected node safely.
    SkipSubtree,
}
