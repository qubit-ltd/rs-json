// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines ordinary JSON tree traversal control.

/// Selects whether a successfully visited node exposes its descendants.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonTreeControl {
    /// Visit the node's descendants in normal depth-first order.
    Descend,
    /// Do not visit or charge the node's descendants.
    SkipSubtree,
}
