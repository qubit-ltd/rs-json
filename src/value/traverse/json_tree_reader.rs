// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Implements non-recursive, read-only JSON tree processing.

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonValueTransaction;
use serde_json::Value;

use super::JsonTreeContext;
use super::JsonTreeLocation;
use super::JsonTreeProcessError;
use super::JsonTreeVisitor;
use super::internal::ChildCursor;
use super::internal::NoopVisitor;
use super::internal::ReadFrame;
use super::internal::ReadFrameState;
use crate::value::internal::json_value_measurement;

/// Processes JSON values while borrowing one staged JSON value transaction.
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
/// use qubit_json::value::traverse::{
///     JsonTreeContext, JsonTreeReader, JsonTreeVisitor,
/// };
/// use serde_json::Value;
///
/// struct Visitor;
/// impl JsonTreeVisitor for Visitor {
///     type Error = std::convert::Infallible;
///
///     fn enter(
///         &mut self,
///         _: &Value,
///         _: JsonTreeContext<'_>,
///     ) -> Result<(), Self::Error> {
///         Ok(())
///     }
/// }
///
/// let mut budget = JsonValueBudget::new(
///     JsonValueLimits::<JsonResource, usize>::default(),
/// );
/// let mut transaction = budget.transaction();
/// let mut reader = JsonTreeReader::new(&mut transaction);
/// assert!(reader.process(&Value::Null, &mut Visitor).is_ok());
/// ```
pub struct JsonTreeReader<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Transaction receiving staged node and payload charges.
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    /// Whether any admission check can reject this traversal.
    enforce_limits: bool,
}

impl<'transaction, 'budget, R, Q> JsonTreeReader<'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a reader borrowing the supplied JSON value transaction.
    ///
    /// # Parameters
    ///
    /// * `transaction` - Transaction receiving node and payload charges.
    ///
    /// # Returns
    ///
    /// A reader borrowing `transaction` for its lifetime.
    #[inline(always)]
    #[must_use]
    pub fn new(transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>) -> Self {
        let enforce_limits = transaction.has_limits();
        Self {
            transaction,
            enforce_limits,
        }
    }

    /// Processes every node in depth-first order without Rust recursion.
    ///
    /// # Type Parameters
    ///
    /// * `V` - Visitor receiving admitted-node callbacks.
    ///
    /// # Parameters
    ///
    /// * `value` - Root JSON value to process.
    /// * `visitor` - Visitor invoked around each admitted node.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the complete tree is processed.
    ///
    /// # Errors
    ///
    /// Returns [`JsonTreeProcessError::Budget`] when resource admission fails,
    /// or [`JsonTreeProcessError::Visitor`] when the visitor rejects a node.
    pub fn process<V>(&mut self, value: &Value, visitor: &mut V) -> Result<(), JsonTreeProcessError<R, Q, V::Error>>
    where
        V: JsonTreeVisitor,
    {
        let mut pending = vec![ReadFrame::enter(
            value,
            JsonTreeContext {
                depth: 1,
                location: JsonTreeLocation::Root,
            },
        )];
        while let Some(frame) = pending.last_mut() {
            match &mut frame.state {
                ReadFrameState::Enter => {
                    let value = frame.value;
                    let context = frame.context;
                    if self.enforce_limits {
                        if let JsonTreeLocation::ObjectValue { key } = context.location {
                            self.transaction.try_admit(JsonMeasurement::Key { bytes: key.len() })?;
                        }
                        self.admit(value, context.depth)?;
                    }
                    visitor.enter(value, context).map_err(JsonTreeProcessError::Visitor)?;
                    frame.state = ReadFrameState::Children(ChildCursor::new(value, context.depth));
                }
                ReadFrameState::Children(cursor) => {
                    if let Some((value, location, depth)) = cursor.next() {
                        pending.push(ReadFrame::enter(value, JsonTreeContext { depth, location }));
                    } else {
                        frame.state = ReadFrameState::Leave;
                    }
                }
                ReadFrameState::Leave => {
                    let frame = pending.pop().expect("read frame exists");
                    visitor
                        .leave(frame.value, frame.context)
                        .map_err(JsonTreeProcessError::Visitor)?;
                }
            }
        }
        Ok(())
    }

    /// Accounts every node and payload without invoking a domain visitor.
    ///
    /// The charges remain staged in the borrowed transaction. The caller
    /// decides whether to commit it after any surrounding work succeeds.
    ///
    /// # Parameters
    ///
    /// * `value` - Root JSON value whose complete tree is admitted.
    ///
    /// # Returns
    ///
    /// `Ok(())` after every node and payload has been staged in the borrowed
    /// transaction.
    ///
    /// # Errors
    ///
    /// Returns the first measured budget rejection encountered during the
    /// traversal.
    pub fn account(&mut self, value: &Value) -> Result<(), MeasuredBudgetError<R, Q>> {
        if !self.enforce_limits {
            return Ok(());
        }
        match self.process(value, &mut NoopVisitor) {
            Ok(()) => Ok(()),
            Err(JsonTreeProcessError::Budget(error)) => Err(error),
            Err(JsonTreeProcessError::Visitor(error)) => match error {},
        }
    }

    /// Admits one node before any visitor callback.
    fn admit(&mut self, value: &Value, depth: usize) -> Result<(), MeasuredBudgetError<R, Q>> {
        self.transaction.try_admit(json_value_measurement(value, depth))
    }
}
