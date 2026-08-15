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
use std::rc::Rc;

use qubit_budget::ResourceQuantity;

use super::json_output_accounting::JsonOutputAccounting;
use crate::text::JsonEncodeError;

/// Writes accepted JSON bytes directly to an external destination.
pub(in crate::text) struct JsonOutputWriter<'a, W, R, Q>
where
    Q: ResourceQuantity,
{
    /// Destination receiving each accepted byte slice.
    writer: W,

    /// Shared output budget and first-failure state.
    accounting: Rc<RefCell<JsonOutputAccounting<'a, R, Q>>>,

    /// A copy of the first destination error for typed conversion after
    /// serde_json has erased it.
    io_error: Option<io::Error>,
}

impl<'a, W, R, Q> JsonOutputWriter<'a, W, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates an incremental writer over shared output accounting.
    pub(in crate::text) const fn new(
        writer: W,
        accounting: Rc<RefCell<JsonOutputAccounting<'a, R, Q>>>,
    ) -> Self {
        Self {
            writer,
            accounting,
            io_error: None,
        }
    }

    /// Converts the serializer result while preserving typed failures.
    pub(in crate::text) fn into_result(
        self,
        result: Result<(), serde_json::Error>,
    ) -> Result<(), JsonEncodeError<R, Q>> {
        let violation = self.accounting.borrow_mut().take_violation();
        if let Some(error) = violation {
            return Err(JsonEncodeError::Budget(error));
        }
        let syntax_error = self.accounting.borrow_mut().take_syntax_error();
        if let Some(error) = syntax_error {
            return Err(JsonEncodeError::InvalidRawJson(error));
        }
        if let Some(error) = self.io_error {
            return Err(JsonEncodeError::Write(error));
        }
        result.map_err(JsonEncodeError::Serialize)
    }
}

impl<W, R, Q> Write for JsonOutputWriter<'_, W, R, Q>
where
    W: Write,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Checks capacity, writes one slice, and charges only bytes accepted by
    /// the destination.
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        {
            let accounting = self.accounting.borrow();
            if let Err(error) = accounting.check_available(input.len()) {
                drop(accounting);
                self.accounting.borrow_mut().record_violation(error);
                return Err(io::Error::other("JSON output budget exceeded"));
            }
        }

        match self.writer.write(input) {
            Ok(written) => {
                if written == 0 && !input.is_empty() {
                    let error = io::Error::new(
                        io::ErrorKind::WriteZero,
                        "JSON output writer accepted no bytes",
                    );
                    self.io_error = Some(io::Error::new(error.kind(), error.to_string()));
                    return Err(error);
                }
                let mut accounting = self.accounting.borrow_mut();
                if let Err(error) = accounting.consume(written) {
                    accounting.record_violation(error);
                    return Err(io::Error::other("JSON output budget exceeded"));
                }
                Ok(written)
            }
            Err(error) => {
                self.io_error = Some(io::Error::new(error.kind(), error.to_string()));
                Err(error)
            }
        }
    }

    /// Flushes the destination and preserves any resulting I/O error.
    fn flush(&mut self) -> io::Result<()> {
        match self.writer.flush() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.io_error = Some(io::Error::new(error.kind(), error.to_string()));
                Err(error)
            }
        }
    }
}
