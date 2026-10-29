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

/// Shared state for the projection transaction and its sticky first failure.
///
/// Collectors borrow this cell so a formatting or budget error is retained by
/// the same [`JsonValueEncodeContext`] used by the caller's serializer.
type ContextCell<'context, 'transaction, 'budget, R, Q> =
    &'context RefCell<JsonValueEncodeContext<'transaction, 'budget, R, Q>>;

/// Collects text emitted by a `Display` implementation while checking its
/// prospective JSON value budget after each write.
///
/// The checks use the cumulative UTF-8 byte length of the text. They are
/// preflight checks through [`JsonValueEncodeContext::check_admit`]; successful
/// string collection is charged by [`super::JsonValueSerializer::string`],
/// while the key path charges its collected text. The shared context retains
/// the first failure, including one caused by a rejected chunk.
pub(in crate::value::json_value_encoder) struct JsonValueDisplayCollector<'context, 'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Shared transaction state and sticky first failure.
    context: ContextCell<'context, 'transaction, 'budget, R, Q>,
    /// Determines whether prospective bytes are measured as a string or key.
    kind: JsonValueDisplayKind,
    /// Text retained from writes that passed their prospective budget check.
    text: String,
}

impl<'context, 'transaction, 'budget, R, Q> JsonValueDisplayCollector<'context, 'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates an empty collector borrowing the caller's projection context.
    ///
    /// The context is shared with the caller so any rejected measurement
    /// becomes the projection's sticky first failure.
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

    /// Returns the collected text, transferring its ownership to the caller.
    ///
    /// This only extracts text: it does not commit admission or charge the
    /// transaction. [`super::JsonValueSerializer::string`] and the key
    /// serializer perform that accounting.
    pub(in crate::value::json_value_encoder) fn into_string(self) -> String {
        self.text
    }

    /// Builds the prospective measurement for the cumulative byte length.
    ///
    /// String measurements include their serialization depth; key
    /// measurements use the key byte count and have no string depth.
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
    /// Checks the cumulative UTF-8 byte length before retaining `chunk`.
    ///
    /// A successful check is only a preflight; the caller charges the complete
    /// string or key once collection succeeds. A rejected check leaves `text`
    /// unchanged and records its error in the shared context. If formatting
    /// then reports `fmt::Error`, the caller must preserve that retained first
    /// failure rather than replacing it with a generic formatting error.
    ///
    /// Returns `fmt::Error` when byte-length arithmetic overflows or the
    /// prospective measurement is rejected.
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
