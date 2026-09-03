// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines callbacks for mutable JSON tree processing.

use serde_json::Value;

use super::JsonTreeContext;
use super::JsonTreeControl;

/// Mutates JSON nodes after the complete input tree has passed admission.
///
/// Output admission runs only after every visitor callback succeeds. Returning
/// [`JsonTreeControl::SkipSubtree`] skips descendant callbacks but does not
/// skip final output accounting.
///
/// # Examples
///
/// ```
/// use qubit_json::value::traverse::{
///     JsonTreeContext, JsonTreeControl, JsonTreeMutVisitor,
/// };
/// use serde_json::Value;
///
/// struct Visitor;
/// impl JsonTreeMutVisitor for Visitor {
///     type Error = std::convert::Infallible;
///
///     fn visit(
///         &mut self,
///         value: &mut Value,
///         _: JsonTreeContext<'_>,
///     ) -> Result<JsonTreeControl, Self::Error> {
///         *value = Value::Null;
///         Ok(JsonTreeControl::SkipSubtree)
///     }
/// }
///
/// let _visitor = Visitor;
/// ```
pub trait JsonTreeMutVisitor {
    /// Domain-specific failure returned by this visitor.
    type Error;

    /// Mutates one node and selects whether descendant callbacks run.
    ///
    /// # Parameters
    ///
    /// * `value` - Current node available for mutation.
    /// * `context` - Root-relative location and depth of the node.
    ///
    /// # Returns
    ///
    /// The traversal control decision for descendant callbacks.
    fn visit(&mut self, value: &mut Value, context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error>;
}
