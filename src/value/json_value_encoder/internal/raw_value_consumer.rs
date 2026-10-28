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

impl<'input> JsonLexicalConsumer<'input> for RawValueConsumer<'_> {
    type Error = JsonValueMaterializationError;

    /// No extra work is needed before a value or key; events drive
    /// construction.
    fn before_value(&mut self, _: usize, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Keys are validated and retained when their lexical event arrives.
    fn before_key(&mut self, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Number tokens are admitted by the builder when their event arrives.
    fn before_number(&mut self, _: &'input str, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Forwards exactly one scanner event; builder errors propagate unchanged.
    fn event(&mut self, event: JsonLexicalEvent<'input>) -> Result<(), Self::Error> {
        self.builder.try_accept(event)
    }
}
