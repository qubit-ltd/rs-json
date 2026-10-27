// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Incremental external writer for budget-aware JSON encoding.

use std::cell::RefCell;
use std::io;
use std::io::Write;

use qubit_budget::ResourceQuantity;
use qubit_io::ByteOutput;

use super::json_output_accounting::JsonOutputAccounting;
use crate::encode::JsonEncodeError;

/// Writes accepted JSON bytes directly to an external destination.
///
/// # Lifetime Parameters
///
/// * `'a` - Lifetime of the shared output accounting borrow.
///
/// # Type Parameters
///
/// * `W` - Destination that accepts byte slices.
/// * `R` - Resource identity used by output accounting.
/// * `Q` - Quantity representation used by output accounting.
pub(in crate::encode) struct JsonOutputWriter<'a, W: ?Sized, R, Q>
where
    Q: ResourceQuantity,
{
    /// Destination receiving each accepted byte slice.
    writer: &'a mut W,

    /// Shared output budget and first-failure state.
    accounting: &'a RefCell<JsonOutputAccounting<'a, R, Q>>,

    /// Whether accepted writes must charge an output-byte budget.
    has_output_budget: bool,

    /// A copy of the first destination error for typed conversion after
    /// serde_json has erased it.
    io_error: Option<io::Error>,
}

impl<'a, W: ?Sized, R, Q> JsonOutputWriter<'a, W, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates an incremental writer over shared output accounting.
    ///
    /// # Parameters
    ///
    /// * `writer` - Destination that receives accepted JSON bytes.
    /// * `accounting` - Shared budget and failure state for this operation.
    ///
    /// # Returns
    ///
    /// A writer that records the first destination error for later conversion.
    #[inline]
    pub(in crate::encode) fn new(writer: &'a mut W, accounting: &'a RefCell<JsonOutputAccounting<'a, R, Q>>) -> Self {
        Self {
            writer,
            accounting,
            has_output_budget: accounting.borrow().has_output_budget(),
            io_error: None,
        }
    }

    /// Records the first destination error and returns a sanitized copy.
    ///
    /// The original error remains available for typed conversion after the
    /// serializer has erased its source. The returned error keeps the same
    /// kind without exposing the destination's error message to serialization.
    fn record_io_error(&mut self, error: io::Error) -> io::Error {
        let kind = error.kind();
        if self.io_error.is_none() {
            self.io_error = Some(error);
        }
        io::Error::new(kind, "JSON output operation failed")
    }

    /// Converts the serializer result while preserving typed failures.
    ///
    /// # Parameters
    ///
    /// * `result` - Result returned by the JSON serializer.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or the budget, syntax, writer, or serialization
    /// failure with its original category preserved.
    ///
    /// # Errors
    ///
    /// Returns the first recorded budget, syntax, or destination error;
    /// otherwise it converts a serializer error to a serialization failure.
    pub(in crate::encode) fn into_result(
        self,
        result: Result<(), serde_json::Error>,
    ) -> Result<(), JsonEncodeError<R, Q>> {
        let violation = self.accounting.borrow_mut().take_violation();
        if let Some(error) = violation {
            return Err(JsonEncodeError::<R, Q>::budget(error));
        }
        let syntax_error = self.accounting.borrow_mut().take_syntax_error();
        if let Some(error) = syntax_error {
            return Err(JsonEncodeError::<R, Q>::invalid_raw_json(error));
        }
        if let Some(error) = self.io_error {
            return Err(JsonEncodeError::<R, Q>::write(error));
        }
        if let Some(error) = self.accounting.borrow_mut().take_serialization_error() {
            return Err(JsonEncodeError::<R, Q>::serialization(error));
        }
        if result.is_err() {
            let error = self.accounting.borrow_mut().take_serialization_error_or_custom();
            return Err(JsonEncodeError::<R, Q>::serialization(error));
        }
        Ok(())
    }
}

impl<O: ?Sized, R, Q> Write for JsonOutputWriter<'_, O, R, Q>
where
    O: ByteOutput,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Checks capacity, writes one slice, and charges only bytes accepted by
    /// the destination.
    ///
    /// # Parameters
    ///
    /// * `input` - Bytes requested by the serializer.
    ///
    /// # Returns
    ///
    /// The number of accepted bytes, or an I/O error when the destination or
    /// output budget rejects the write.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the destination fails, writes zero bytes for
    /// a non-empty input, or the output budget rejects the requested bytes.
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if self.has_output_budget {
            let accounting = self.accounting.borrow();
            if let Err(error) = accounting.check_available(input.len()) {
                drop(accounting);
                self.accounting.borrow_mut().record_violation(error);
                return Err(io::Error::other("JSON output budget exceeded"));
            }
        }

        match qubit_io::Output::write(&mut *self.writer, input) {
            Ok(written) => {
                if written == 0 && !input.is_empty() {
                    let error = io::Error::new(io::ErrorKind::WriteZero, "JSON output writer accepted no bytes");
                    return Err(self.record_io_error(error));
                }
                if self.has_output_budget {
                    let mut accounting = self.accounting.borrow_mut();
                    if let Err(error) = accounting.consume(written) {
                        accounting.record_violation(error);
                        return Err(io::Error::other("JSON output budget exceeded"));
                    }
                }
                Ok(written)
            }
            Err(error) => {
                if error.kind() == io::ErrorKind::Interrupted {
                    Err(error)
                } else {
                    Err(self.record_io_error(error))
                }
            }
        }
    }

    /// Flushes the destination and preserves any resulting I/O error.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the destination flushes, or its I/O error otherwise.
    ///
    /// # Errors
    ///
    /// Returns the destination's flush error.
    fn flush(&mut self) -> io::Result<()> {
        match qubit_io::Output::flush(&mut *self.writer) {
            Ok(()) => Ok(()),
            Err(error) => Err(self.record_io_error(error)),
        }
    }
}
