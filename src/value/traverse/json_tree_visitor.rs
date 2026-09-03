// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines callbacks for a read-only JSON tree traversal.

use serde_json::Value;

use super::JsonTreeContext;

/// Receives enter and leave events for budget-admitted JSON tree nodes.
///
/// # Examples
///
/// ```
/// use qubit_json::value::traverse::{
///     JsonTreeContext, JsonTreeVisitor,
/// };
/// use serde_json::Value;
///
/// struct CountingVisitor {
///     count: usize,
/// }
/// impl JsonTreeVisitor for CountingVisitor {
///     type Error = std::convert::Infallible;
///
///     fn enter(
///         &mut self,
///         _: &Value,
///         _: JsonTreeContext<'_>,
///     ) -> Result<(), Self::Error> {
///         self.count += 1;
///         Ok(())
///     }
/// }
///
/// let _visitor = CountingVisitor { count: 0 };
/// ```
pub trait JsonTreeVisitor {
    /// Domain-specific failure returned by this visitor.
    type Error;

    /// Handles a node after its budget admission and before its descendants.
    ///
    /// # Parameters
    ///
    /// * `value` - Admitted node being visited.
    /// * `context` - Root-relative location and depth of the node.
    ///
    /// # Returns
    ///
    /// `Ok(())` to continue traversal, or the visitor's error to stop it.
    fn enter(&mut self, value: &Value, context: JsonTreeContext<'_>) -> Result<(), Self::Error>;

    /// Handles a node after all of its descendants have been handled.
    ///
    /// # Parameters
    ///
    /// * `value` - Admitted node whose descendants have been visited.
    /// * `context` - Root-relative location and depth of the node.
    ///
    /// # Returns
    ///
    /// `Ok(())` to continue traversal, or the visitor's error to stop it.
    fn leave(&mut self, _value: &Value, _context: JsonTreeContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
}
