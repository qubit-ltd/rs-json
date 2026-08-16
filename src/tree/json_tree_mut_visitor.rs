// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines callbacks for mutable JSON tree processing.

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use serde_json::Value;

use super::JsonTreeBudgetRejection;
use super::JsonTreeContext;
use super::JsonTreeControl;

/// Mutates admitted JSON nodes and handles optional fail-closed budget
/// rejection.
pub trait JsonTreeMutVisitor<R, Q>
where
    Q: ResourceQuantity,
{
    /// Domain-specific failure returned by this visitor.
    type Error;

    /// Handles an admitted node and selects whether its descendants are
    /// visited.
    ///
    /// # Parameters
    ///
    /// * `value` - Admitted node available for mutation.
    /// * `context` - Root-relative location and depth of the node.
    ///
    /// # Returns
    ///
    /// The traversal control decision for the node.
    fn visit(
        &mut self,
        value: &mut Value,
        context: JsonTreeContext<'_>,
    ) -> Result<JsonTreeControl, Self::Error>;

    /// Handles an unadmitted node before its subtree is skipped or processing
    /// aborts.
    ///
    /// # Parameters
    ///
    /// * `value` - Node whose admission was rejected.
    /// * `context` - Root-relative location and depth of the node.
    /// * `error` - Measured reason why the node was not admitted.
    ///
    /// # Returns
    ///
    /// A rejection policy indicating whether processing should abort or skip
    /// the rejected subtree.
    fn reject_budget(
        &mut self,
        _value: &mut Value,
        _context: JsonTreeContext<'_>,
        _error: &MeasuredBudgetError<R, Q>,
    ) -> Result<JsonTreeBudgetRejection, Self::Error> {
        Ok(JsonTreeBudgetRejection::Abort)
    }
}
