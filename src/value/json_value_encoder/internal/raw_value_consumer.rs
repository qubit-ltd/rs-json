// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Transaction-aware Serde compound serializers.

use crate::lexical::JsonLexicalConsumer;
use crate::lexical::JsonLexicalEvent;
use crate::value::internal::JsonValueMaterializationError;
use crate::value::internal::LexicalJsonValueBuilder;

/// Forwards lexical events into the guarded raw-value builder.
pub(in crate::value::json_value_encoder) struct RawValueConsumer<'builder> {
    /// Builder that owns the partial tree and cleans it up on failure.
    pub(in crate::value::json_value_encoder) builder: &'builder mut LexicalJsonValueBuilder,
}

/// Connects lexical scanner callbacks to the guarded value builder.
///
/// The pre-event checks are intentionally no-ops because this builder validates
/// and records the corresponding data when it receives each event. Event
/// failures are returned to the scanner unchanged.
impl<'input> JsonLexicalConsumer<'input> for RawValueConsumer<'_> {
    /// Reports materialization failures produced while accepting lexical
    /// events.
    type Error = JsonValueMaterializationError;

    /// No extra work is needed before a value or key; events drive
    /// construction.
    ///
    /// # Parameters
    ///
    /// * `depth` — root-inclusive nesting depth supplied by the scanner.
    /// * `offset` — zero-based byte offset of the value in the input.
    fn before_value(&mut self, _depth: usize, _offset: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Keys are validated and retained when their lexical event arrives.
    ///
    /// # Parameters
    ///
    /// * `offset` — zero-based byte offset of the key token in the input.
    fn before_key(&mut self, _offset: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Number tokens are admitted by the builder when their event arrives.
    ///
    /// # Parameters
    ///
    /// * `token` — number spelling borrowed from the input for `'input`.
    /// * `offset` — zero-based byte offset of the number token in the input.
    fn before_number(&mut self, _token: &'input str, _offset: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Forwards exactly one scanner event; builder errors propagate unchanged.
    ///
    /// # Parameters
    ///
    /// * `event` — recognized lexical event with token borrows tied to
    ///   `'input`.
    fn event(&mut self, event: JsonLexicalEvent<'input>) -> Result<(), Self::Error> {
        self.builder.try_accept(event)
    }
}
