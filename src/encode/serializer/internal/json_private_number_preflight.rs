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

use super::super::json_encode_context::JsonEncodeContext;
use crate::internal::StrictJsonNumber;

/// Checks output capacity, value budget, and strict JSON number syntax.
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
