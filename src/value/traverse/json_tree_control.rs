// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines control returned by mutable JSON tree visitors.

/// Selects whether a mutable tree visitor receives callbacks for descendants.
///
/// This control is returned by [`super::JsonTreeMutVisitor`] and interpreted by
/// [`super::JsonTreeMutator::process`]. Reader APIs do not return this control.
///
/// # Examples
///
/// ```
/// use qubit_json::value::traverse::JsonTreeControl;
///
/// let control = JsonTreeControl::SkipSubtree;
/// assert_eq!(control, JsonTreeControl::SkipSubtree);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonTreeControl {
    /// Visit the node's descendants in normal depth-first order.
    Descend,
    /// Skip callbacks for the node's descendants.
    ///
    /// Descendants remain subject to complete input and final output budget
    /// accounting performed by [`super::JsonTreeMutator::process`].
    SkipSubtree,
}
