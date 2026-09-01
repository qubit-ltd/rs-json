// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Bounded output buffer for budget-aware JSON encoding.
// qubit-style: allow source-test-pair
// qubit-style: allow multiple-public-types

use std::cell::RefCell;
use std::io;
use std::io::Write;

use qubit_budget::ResourceQuantity;

use super::json_output_accounting::JsonOutputAccounting;
use crate::encode::JsonEncodeError;

/// Accumulates JSON bytes only while the configured output budget permits it.
pub(in crate::encode) struct JsonOutputBuffer<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Bytes accepted by the in-memory buffer so far.
    bytes: Vec<u8>,

    /// Accounting shared with the online serializer checks.
    accounting: &'a RefCell<JsonOutputAccounting<'a, R, Q>>,

    /// Operation-local capacity used by the successful write hot path.
    remaining: Option<Q>,
}

impl<'a, R, Q> JsonOutputBuffer<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates an empty bounded output buffer.
    ///
    /// # Parameters
    ///
    /// * `accounting` - Shared live budget and first-violation state.
    ///
    /// # Returns
    ///
    /// An empty writer with no recorded violation.
    #[inline]
    pub(in crate::encode) fn new(accounting: &'a RefCell<JsonOutputAccounting<'a, R, Q>>) -> Self {
        Self {
            bytes: Vec::new(),
            accounting,
            remaining: accounting.borrow().remaining(),
        }
    }

    /// Resolves serialization and returns the accepted bytes or original error.
    ///
    /// # Parameters
    ///
    /// * `result` - Result returned by the JSON serializer.
    /// # Returns
    ///
    /// The complete bounded output when serialization succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`JsonEncodeError::Budget`] for a recorded output violation,
    /// taking precedence over its erased I/O representation. Otherwise returns
    /// [`JsonEncodeError::Serialize`] for the serializer failure.
    pub(in crate::encode) fn into_result(
        self,
        result: Result<(), serde_json::Error>,
    ) -> Result<Vec<u8>, JsonEncodeError<R, Q>> {
        let violation = self.accounting.borrow_mut().take_violation();
        if let Some(error) = violation {
            return Err(JsonEncodeError::Budget(error));
        }
        let syntax_error = self.accounting.borrow_mut().take_syntax_error();
        if let Some(error) = syntax_error {
            return Err(JsonEncodeError::InvalidRawJson(error));
        }
        if result.is_err() {
            let error = self.accounting.borrow_mut().take_serialization_error_or_custom();
            return Err(JsonEncodeError::Serialize(error));
        }
        Ok(self.bytes)
    }
}

impl<R, Q> Write for JsonOutputBuffer<'_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Appends one complete input slice after checking the resulting length.
    ///
    /// The buffer does not charge the output budget. Its caller consumes the
    /// complete length only after successful serialization, so a failed Vec or
    /// buffered-writer encode leaves output usage unchanged.
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let next = match self.bytes.len().checked_add(input.len()) {
            Some(next) => next,
            None => {
                return Err(io::Error::other("JSON output length overflow"));
            }
        };
        if let Some(remaining) = self.remaining {
            let amount = Q::try_from_usize(input.len());
            match amount {
                Ok(amount) if amount <= remaining => {
                    self.remaining = Some(remaining - amount);
                }
                Ok(_) | Err(_) => {
                    let error = self
                        .accounting
                        .borrow()
                        .check_available(next)
                        .expect_err("the local output capacity already rejected this write");
                    self.accounting.borrow_mut().record_violation(error);
                    return Err(io::Error::other("JSON output budget exceeded"));
                }
            }
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }

    /// Flushes the in-memory buffer without performing external I/O.
    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
