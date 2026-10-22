// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Non-recursive lexical admission for one JSON input document.

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonValueTransaction;

use super::json_lexical_container_frame::JsonLexicalContainerFrame;
use super::json_lexical_cursor::JsonLexicalCursor;
use super::json_lexical_error::JsonLexicalError;
use super::json_lexical_error_reason::JsonLexicalErrorReason;
use super::json_lexical_event::JsonLexicalEvent;
use super::json_number_model::JsonNumberModel;

/// Lexically validates and charges one JSON document without recursion.
pub(crate) struct JsonLexicalScanner<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// JSON value resources charged while scanning the document.
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    /// Root-inclusive depth assigned to the scanned value.
    root_depth: usize,
    /// Whether scanning must stage JSON value measurements.
    has_value_limits: bool,
    /// Numeric range contract selected by the owning decoder.
    number_model: JsonNumberModel,
}

impl<'transaction, 'budget, R, Q> JsonLexicalScanner<'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a lexical scanner bound to one value transaction.
    #[inline(always)]
    pub(crate) const fn new(
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
        has_value_limits: bool,
    ) -> Self {
        Self {
            transaction,
            root_depth: 1,
            has_value_limits,
            number_model: JsonNumberModel::Integer64,
        }
    }

    /// Creates a lexical scanner rooted at an enclosing serializer depth.
    #[inline(always)]
    pub(crate) const fn at_depth(
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
        root_depth: usize,
        has_value_limits: bool,
    ) -> Self {
        Self {
            transaction,
            root_depth,
            has_value_limits,
            number_model: JsonNumberModel::Integer64,
        }
    }

    /// Selects the number range contract for this scan.
    #[inline(always)]
    pub(crate) const fn with_number_model(mut self, number_model: JsonNumberModel) -> Self {
        self.number_model = number_model;
        self
    }

    /// Validates and charges one complete JSON document.
    ///
    /// # Errors
    ///
    /// Returns [`JsonLexicalError::Budget`] for the first resource violation,
    /// or [`JsonLexicalError::Syntax`] when `input` is not one complete JSON
    /// value. All value measurements remain staged in the caller's transaction.
    pub(crate) fn scan(&mut self, input: &[u8]) -> Result<(), JsonLexicalError<R, Q>> {
        self.scan_with_events(input, |_| {})
    }

    /// Validates a document and synchronously emits each admitted token.
    ///
    /// Events may precede a later document error; consumers must only
    /// materialize input after a complete admission pass has succeeded.
    pub(crate) fn scan_with_events<'input, F>(
        &mut self,
        input: &'input [u8],
        mut emit: F,
    ) -> Result<(), JsonLexicalError<R, Q>>
    where
        F: FnMut(JsonLexicalEvent<'input>),
    {
        let mut cursor = JsonLexicalCursor::new(input, &mut *self.transaction, self.has_value_limits);
        cursor.set_number_model(self.number_model);
        let mut stack: Vec<JsonLexicalContainerFrame> = Vec::new();
        cursor.skip_whitespace();
        cursor.value(self.root_depth, &mut stack, &mut emit)?;
        while let Some(frame) = stack.pop() {
            cursor.resume(frame, &mut stack, &mut emit)?;
        }
        cursor.skip_whitespace();
        if cursor.is_at_end() {
            Ok(())
        } else {
            Err(cursor.syntax(JsonLexicalErrorReason::TrailingCharacters))
        }
    }
}
