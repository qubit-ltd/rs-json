// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Preflights serde_json's private number text before raw-value emission.

use std::cell::RefCell;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonMeasurement;
use serde::ser::Error;

use super::JsonEncodeContext;
use crate::internal::StrictJsonNumber;

/// Checks output capacity, value budget, and strict JSON number syntax before
/// serde_json emits its private number text as raw JSON.
///
/// The output budget is checked before syntax validation. The value budget is
/// checked and consumed only when the encode context enables value limits.
/// Invalid number text is recorded as a structured serialization failure and
/// returned through `E`.
///
/// # Type Parameters
///
/// - `E`: Serializer error type used to report rejected input or budgets.
/// - `R`: Resource identifier type stored by the encode context.
/// - `Q`: Quantity type used by resource accounting.
///
/// # Parameters
///
/// - `context`: Mutable operation state containing output and optional value
///   budgets; interior mutability allows the serializer callback to update it.
/// - `text`: Number text that serde_json is about to emit.
/// - `depth`: Container depth to associate with this number in value
///   accounting.
///
/// # Returns
///
/// Returns `Ok(())` after syntax validation and admission to the enabled
/// budgets.
///
/// # Errors
///
/// Returns an `E` when output or value accounting rejects the number, or when
/// `text` is not valid strict JSON number syntax.
///
/// # Panics
///
/// Panics if `context` or its output accounting cell is already borrowed when
/// this function accesses it.
pub(in crate::encode::serializer) fn preflight<E, R, Q>(
    context: &RefCell<JsonEncodeContext<'_, '_, R, Q>>,
    text: &str,
    depth: usize,
) -> Result<(), E>
where
    E: Error,
    R: Clone,
    Q: ResourceQuantity,
{
    let output_check = context.borrow().output.borrow().check_available(text.len());
    context.borrow_mut().record(output_check)?;

    let measurement = JsonMeasurement::Number {
        depth,
        bytes: text.len(),
    };
    if context.borrow().has_value_limits {
        let check = context.borrow_mut().transaction.check_admit(measurement);
        context.borrow_mut().record(check)?;
    }

    if let Err(error) = StrictJsonNumber::parse(text) {
        context
            .borrow_mut()
            .output
            .borrow_mut()
            .record_serialization_error(error);
        return Err(E::custom("invalid JSON number"));
    }

    context.borrow_mut().admit(measurement)
}
