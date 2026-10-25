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
/// If either callback returns an error, the reader stops immediately and does
/// not guarantee a matching `leave` for nodes already entered. Visitor state
/// and external side effects are not restored; callers that require paired
/// callbacks should discard and rebuild the visitor after an error.
///
/// # Examples
///
/// ```
/// use std::convert::Infallible;
///
/// use qubit_budget::json::JsonResource;
/// use qubit_budget::json::JsonValueLimits;
/// use qubit_json::value::traverse::JsonTreeContext;
/// use qubit_json::value::traverse::JsonTreeReader;
/// use qubit_json::value::traverse::JsonTreeVisitor;
/// use serde_json::Value;
/// use serde_json::json;
///
/// struct CountingVisitor {
///     entered: usize,
///     left: usize,
/// }
///
/// impl JsonTreeVisitor for CountingVisitor {
///     type Error = Infallible;
///
///     fn enter(&mut self, _: &Value, _: JsonTreeContext<'_>) -> Result<(), Self::Error> {
///         self.entered += 1;
///         Ok(())
///     }
///
///     fn leave(&mut self, _: &Value, _: JsonTreeContext<'_>) -> Result<(), Self::Error> {
///         self.left += 1;
///         Ok(())
///     }
/// }
///
/// let limits = JsonValueLimits::<JsonResource, usize>::builder().max_nodes(4).build();
/// let mut budget = limits.budget();
/// let mut transaction = budget.transaction();
/// let mut visitor = CountingVisitor { entered: 0, left: 0 };
/// let value = json!({"items": [true, null]});
/// JsonTreeReader::new(&mut transaction).process(&value, &mut visitor).unwrap();
/// assert_eq!((visitor.entered, visitor.left), (4, 4));
/// transaction.commit().unwrap();
/// assert_eq!(budget.used_nodes(), Some(4));
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
    ///
    /// # Errors
    ///
    /// Returns the visitor-defined error when handling this node fails.
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
    ///
    /// # Errors
    ///
    /// Returns the visitor-defined error when handling this node fails.
    fn leave(&mut self, _value: &Value, _context: JsonTreeContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
}
