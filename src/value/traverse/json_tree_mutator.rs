// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Implements non-recursive, mutable JSON tree processing.

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonValueTransaction;
use serde_json::Value;

use super::JsonTreeBudgetRejection;
use super::JsonTreeControl;
use super::JsonTreeMutVisitor;
use super::JsonTreeProcessError;
use super::internal::MutFrame;
use crate::value::internal::json_number_lexeme_length;

/// Mutates JSON values while borrowing one staged JSON value transaction.
///
/// The transaction accounts each input node immediately before its visitor
/// callback. Mutations made by the visitor, including replacement values and
/// newly introduced descendants, are not re-accounted; callers needing an
/// output budget should restrict visitors to structural reductions or account
/// the completed tree separately.
///
/// # Type Parameters
///
/// * `R` - Resource identity tracked by the borrowed transaction.
/// * `Q` - Quantity representation used for resource accounting.
///
/// # Examples
///
/// ```
/// use qubit_budget::json::{JsonResource, JsonValueBudget, JsonValueLimits};
/// use qubit_json::value::traverse::{JsonTreeContext, JsonTreeControl, JsonTreeMutVisitor, JsonTreeMutator};
/// use serde_json::Value;
///
/// struct Visitor;
/// impl JsonTreeMutVisitor<JsonResource, usize> for Visitor {
///     type Error = std::convert::Infallible;
///
///     fn visit(&mut self, _: &mut Value, _: JsonTreeContext<'_>) ->
/// Result<JsonTreeControl, Self::Error> {
///         Ok(JsonTreeControl::SkipSubtree)
///     }
/// }
///
/// let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource,
/// usize>::default()); let mut transaction = budget.transaction();
/// let mut mutator = JsonTreeMutator::new(&mut transaction);
/// let mut value = Value::Null;
/// assert!(mutator.process(&mut value, &mut Visitor).is_ok());
/// ```
pub struct JsonTreeMutator<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
}

impl<'transaction, 'budget, R, Q> JsonTreeMutator<'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a mutator borrowing the supplied JSON value transaction.
    ///
    /// # Parameters
    ///
    /// * `transaction` - Transaction receiving node and payload charges.
    ///
    /// # Returns
    ///
    /// A mutator borrowing `transaction` for its lifetime.
    #[inline(always)]
    #[must_use]
    pub fn new(transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>) -> Self {
        Self { transaction }
    }

    /// Mutates every admitted input node in depth-first order without Rust
    /// recursion.
    ///
    /// A node is charged before `visitor.visit` receives it. Consequently, a
    /// visitor replacement is not charged again and only descendants present
    /// after the callback are traversed.
    ///
    /// # Type Parameters
    ///
    /// * `V` - Visitor receiving mutable admitted-node callbacks.
    ///
    /// # Parameters
    ///
    /// * `root` - Root JSON value to process and mutate.
    /// * `visitor` - Visitor controlling mutations and descendant traversal.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the complete tree is processed.
    ///
    /// # Errors
    ///
    /// Returns [`JsonTreeProcessError::Budget`] when resource admission fails,
    /// or [`JsonTreeProcessError::Visitor`] when the visitor rejects a node.
    pub fn process<V>(&mut self, root: &mut Value, visitor: &mut V) -> Result<(), JsonTreeProcessError<R, Q, V::Error>>
    where
        V: JsonTreeMutVisitor<R, Q>,
    {
        let mut stack = vec![MutFrame::root(root)];
        while !stack.is_empty() {
            let index = stack.len() - 1;
            if !stack[index].entered {
                let frame = &mut stack[index];
                let context = frame.location.context(frame.depth);
                // SAFETY: frames are made from the caller's exclusive root
                // borrow; no ancestor container is structurally
                // changed while a child is live.
                let value = unsafe { frame.value.as_mut() };
                if let Some(key) = frame.location.key()
                    && let Err(error) = self.transaction.try_admit(JsonMeasurement::Key { bytes: key.len() })
                {
                    let rejection = visitor
                        .reject_budget(value, context, &error)
                        .map_err(JsonTreeProcessError::Visitor)?;
                    if rejection == JsonTreeBudgetRejection::Abort {
                        return Err(JsonTreeProcessError::Budget(error));
                    }
                    frame.entered = true;
                    frame.finished = true;
                    continue;
                }
                if let Err(error) = self.admit(value, frame.depth) {
                    let rejection = visitor
                        .reject_budget(value, context, &error)
                        .map_err(JsonTreeProcessError::Visitor)?;
                    if rejection == JsonTreeBudgetRejection::Abort {
                        return Err(JsonTreeProcessError::Budget(error));
                    }
                    frame.entered = true;
                    frame.finished = true;
                    continue;
                }
                let control = visitor.visit(value, context).map_err(JsonTreeProcessError::Visitor)?;
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

    /// Admits one node before invoking its mutable visitor callback.
    fn admit(&mut self, value: &Value, depth: usize) -> Result<(), MeasuredBudgetError<R, Q>> {
        let measurement = match value {
            Value::Null => JsonMeasurement::Null { depth },
            Value::Bool(_) => JsonMeasurement::Boolean { depth },
            Value::Number(number) => JsonMeasurement::Number {
                depth,
                bytes: json_number_lexeme_length(number),
            },
            Value::String(text) => JsonMeasurement::String {
                depth,
                bytes: text.len(),
            },
            Value::Array(values) => JsonMeasurement::Array {
                depth,
                items: values.len(),
            },
            Value::Object(entries) => JsonMeasurement::Object {
                depth,
                entries: entries.len(),
            },
        };
        self.transaction.try_admit(measurement)
    }
}
