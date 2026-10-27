// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Display collector that checks value budget before retaining each chunk.

use std::cell::RefCell;
use std::fmt;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonMeasurement;

use super::JsonValueDisplayKind;
use super::JsonValueEncodeContext;
use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;

type ContextCell<'context, 'transaction, 'budget, R, Q> =
    &'context RefCell<JsonValueEncodeContext<'transaction, 'budget, R, Q>>;

pub(in crate::value::json_value_encoder) struct JsonValueDisplayCollector<'context, 'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    context: ContextCell<'context, 'transaction, 'budget, R, Q>,
    kind: JsonValueDisplayKind,
    text: String,
}

impl<'context, 'transaction, 'budget, R, Q> JsonValueDisplayCollector<'context, 'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    pub(in crate::value::json_value_encoder) fn new(
        context: ContextCell<'context, 'transaction, 'budget, R, Q>,
        kind: JsonValueDisplayKind,
    ) -> Self {
        Self {
            context,
            kind,
            text: String::new(),
        }
    }

    pub(in crate::value::json_value_encoder) fn into_string(self) -> String {
        self.text
    }

    fn measurement(&self, bytes: usize) -> JsonMeasurement {
        match self.kind {
            JsonValueDisplayKind::String { depth } => JsonMeasurement::String { depth, bytes },
            JsonValueDisplayKind::Key => JsonMeasurement::Key { bytes },
        }
    }
}

impl<R, Q> fmt::Write for JsonValueDisplayCollector<'_, '_, '_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    fn write_str(&mut self, chunk: &str) -> fmt::Result {
        let Some(next) = self.text.len().checked_add(chunk.len()) else {
            let error = JsonSerializationError::new(JsonSerializationErrorKind::DisplayFormattingFailed);
            let _ = self.context.borrow_mut().record_serialization_error(error);
            return Err(fmt::Error);
        };
        let result = { self.context.borrow_mut().check_admit(self.measurement(next)) };
        if result.is_err() {
            return Err(fmt::Error);
        }
        self.text.push_str(chunk);
        Ok(())
    }
}
