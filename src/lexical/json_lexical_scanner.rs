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
        }
    }

    /// Validates and charges one complete JSON document.
    ///
    /// # Errors
    ///
    /// Returns [`JsonLexicalError::Budget`] for the first resource violation,
    /// or [`JsonLexicalError::Syntax`] when `input` is not one complete JSON
    /// value. All value measurements remain staged in the caller's transaction.
    pub(crate) fn scan(&mut self, input: &[u8]) -> Result<(), JsonLexicalError<R, Q>> {
        let mut cursor = JsonLexicalCursor::new(input, &mut *self.transaction, self.has_value_limits);
        let mut stack: Vec<JsonLexicalContainerFrame> = Vec::new();
        cursor.skip_whitespace();
        cursor.value(self.root_depth, &mut stack)?;
        while let Some(frame) = stack.pop() {
            cursor.resume(frame, &mut stack)?;
        }
        cursor.skip_whitespace();
        if cursor.is_at_end() {
            Ok(())
        } else {
            Err(cursor.syntax(JsonLexicalErrorReason::TrailingCharacters))
        }
    }
}
