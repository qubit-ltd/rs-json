// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fallible callbacks for the shared lexical JSON scanner.

use super::JsonLexicalEvent;

/// Receives scanner checks before token events are emitted.
///
/// Value depth includes the document root, whose depth is one; all offsets are
/// zero-based byte offsets into the original input. For each value, the scanner
/// calls `before_value` before its event, and calls `before_number` after
/// `before_value` but before a number event. Object keys similarly pass through
/// `before_key` before their key event. If any callback returns an error, the
/// scanner stops immediately and emits no later callbacks; side effects already
/// performed by callbacks are not rolled back.
pub(crate) trait JsonLexicalConsumer<'input> {
    type Error;

    fn before_value(&mut self, depth: usize, offset: usize) -> Result<(), Self::Error>;
    fn before_key(&mut self, offset: usize) -> Result<(), Self::Error>;
    fn before_number(&mut self, token: &'input str, offset: usize) -> Result<(), Self::Error>;
    fn event(&mut self, event: JsonLexicalEvent<'input>) -> Result<(), Self::Error>;
}

/// One callback point dispatched in source scan order.
pub(crate) enum JsonLexicalScanPoint<'input> {
    BeforeValue { depth: usize, offset: usize },
    BeforeKey { offset: usize },
    BeforeNumber { token: &'input str, offset: usize },
    Event(JsonLexicalEvent<'input>),
}
