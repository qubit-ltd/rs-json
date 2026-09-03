// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Implements non-recursive, budget-aware mutable JSON tree processing.

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonValueTransaction;
use serde_json::Value;

use super::JsonTreeControl;
use super::JsonTreeMutVisitor;
use super::JsonTreeMutateError;
use super::JsonTreeReader;
use super::internal::MutFrame;

/// Mutates a JSON tree between independent input and output transactions.
///
/// The complete original tree is admitted before the first visitor callback.
/// After all callbacks succeed, the complete mutated tree is admitted against
/// the output transaction. Visitor failures and panics can retain partial
/// mutations, while output accounting does not begin until visitor success.
///
/// # Type Parameters
///
/// * `R` - Resource identity shared by the two transactions.
/// * `Q` - Quantity representation shared by the two transactions.
///
/// # Examples
///
/// ```
/// use qubit_budget::json::{JsonResource, JsonValueBudget, JsonValueLimits};
/// use qubit_json::value::traverse::{
///     JsonTreeContext, JsonTreeControl, JsonTreeMutVisitor, JsonTreeMutator,
/// };
/// use serde_json::Value;
///
/// struct Visitor;
/// impl JsonTreeMutVisitor for Visitor {
///     type Error = std::convert::Infallible;
///
///     fn visit(
///         &mut self,
///         _: &mut Value,
///         _: JsonTreeContext<'_>,
///     ) -> Result<JsonTreeControl, Self::Error> {
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
/// let mut value = Value::Null;
/// assert!(mutator.process(&mut value, &mut Visitor).is_ok());
/// ```
pub struct JsonTreeMutator<'input_transaction, 'input_budget, 'output_transaction, 'output_budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Transaction receiving the complete original-tree charges.
    input: &'input_transaction mut JsonValueTransaction<'input_budget, R, Q>,
    /// Transaction receiving the complete mutated-tree charges.
    output: &'output_transaction mut JsonValueTransaction<'output_budget, R, Q>,
}

impl<'input_transaction, 'input_budget, 'output_transaction, 'output_budget, R, Q>
    JsonTreeMutator<'input_transaction, 'input_budget, 'output_transaction, 'output_budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a mutator borrowing independent input and output transactions.
    ///
    /// # Parameters
    ///
    /// * `input` - Transaction receiving charges for the complete original
    ///   tree.
    /// * `output` - Transaction receiving charges for the complete mutated
    ///   tree.
    ///
    /// # Returns
    ///
    /// A mutator borrowing both transactions for its lifetime.
    #[inline(always)]
    #[must_use]
    pub fn new(
        input: &'input_transaction mut JsonValueTransaction<'input_budget, R, Q>,
        output: &'output_transaction mut JsonValueTransaction<'output_budget, R, Q>,
    ) -> Self {
        Self { input, output }
    }

    /// Admits the original tree, mutates it, and admits the final tree.
    ///
    /// Both admissions and all visitor callbacks use explicit stacks rather
    /// than Rust recursion. `SkipSubtree` affects callbacks only; final output
    /// admission always covers every resulting descendant.
    ///
    /// # Type Parameters
    ///
    /// * `V` - Visitor controlling mutations and descendant callbacks.
    ///
    /// # Parameters
    ///
    /// * `root` - Root JSON value to process and mutate.
    /// * `visitor` - Visitor applied after complete input admission.
    ///
    /// # Returns
    ///
    /// `Ok(())` after both complete trees fit and every callback succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`JsonTreeMutateError::InputBudget`] before mutation when the
    /// original tree is rejected, [`JsonTreeMutateError::Visitor`] after a
    /// callback failure, or [`JsonTreeMutateError::OutputBudget`] after a
    /// complete mutation whose result is rejected. Visitor and output failures
    /// retain mutations already made to `root`.
    pub fn process<V>(&mut self, root: &mut Value, visitor: &mut V) -> Result<(), JsonTreeMutateError<R, Q, V::Error>>
    where
        V: JsonTreeMutVisitor,
    {
        JsonTreeReader::new(&mut *self.input)
            .account(root)
            .map_err(JsonTreeMutateError::InputBudget)?;
        Self::mutate(root, visitor).map_err(JsonTreeMutateError::Visitor)?;
        JsonTreeReader::new(&mut *self.output)
            .account(root)
            .map_err(JsonTreeMutateError::OutputBudget)
    }

    /// Runs mutable visitor callbacks without budget side effects.
    fn mutate<V>(root: &mut Value, visitor: &mut V) -> Result<(), V::Error>
    where
        V: JsonTreeMutVisitor,
    {
        let mut stack = vec![MutFrame::root(root)];
        while !stack.is_empty() {
            let index = stack.len() - 1;
            if !stack[index].entered {
                let frame = &mut stack[index];
                let context = frame.location.context(frame.depth);
                // SAFETY: frames originate from the caller's exclusive root
                // borrow. No ancestor container is accessed while a child
                // frame is live, and each frame is removed before its parent
                // resumes structural traversal.
                let value = unsafe { frame.value.as_mut() };
                let control = visitor.visit(value, context)?;
                frame.entered = true;
                if control != JsonTreeControl::Descend {
                    frame.finished = true;
                }
                continue;
            }
            if let Some(child) = stack[index].next_child() {
                stack.push(child);
                continue;
            }
            let _ = stack.pop().expect("mutable frame exists");
        }
        Ok(())
    }
}
