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
/// Callbacks can visit descendants added by earlier callbacks. Output
/// `max_nodes` and `max_depth` apply only to the complete final tree; they do
/// not bound callback count or intermediate expansion. A visitor that adds
/// descendants must enforce its own expansion bound and ensure traversal
/// terminates. `SkipSubtree` skips descent from the current node, but does not
/// exempt the resulting tree from
/// final output admission. See the
/// [mutable tree traversal guidance](https://github.com/qubit-ltd/rs-json/blob/main/doc/user_guide.md#read-and-mutate-existing-trees).
/// [`crate::value::traverse::JsonTreeMutator::process`] supplies the context to
/// each callback during traversal.
///
/// # Examples
///
/// ```
/// use qubit_budget::json::{JsonResource, JsonValueBudget, JsonValueLimits};
/// use qubit_json::value::traverse::{
///     JsonTreeContext, JsonTreeControl, JsonTreeMutator, JsonTreeMutVisitor,
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
/// let limits = JsonValueLimits::<JsonResource, usize>::default();
/// let mut input_budget = JsonValueBudget::new(limits);
/// let mut output_budget = JsonValueBudget::new(limits);
/// let mut input = input_budget.transaction();
/// let mut output = output_budget.transaction();
/// let mut mutator = JsonTreeMutator::new(&mut input, &mut output);
/// let mut visitor = Visitor;
/// let mut value = Value::Bool(true);
/// mutator.process(&mut value, &mut visitor).unwrap();
/// assert_eq!(value, Value::Null);
/// ```
pub trait JsonTreeMutVisitor {
    /// Domain-specific failure returned by this visitor.
    type Error;

    /// Mutates one node and selects whether descendant callbacks run.
    ///
    /// # Parameters
    ///
    /// * `value` - Current node available for mutation.
    /// * `context` - The node's immediate-parent location and root-inclusive
    ///   depth.
    ///
    /// # Returns
    ///
    /// The traversal control decision for descendant callbacks.
    ///
    /// # Errors
    ///
    /// Returns the visitor's [`Self::Error`] when the visitor cannot process
    /// the current node.
    fn visit(&mut self, value: &mut Value, context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error>;
}
