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
    /// Error returned when a callback stops scanning.
    type Error;

    /// Checks a value before its event, at its zero-based byte offset and
    /// root-inclusive depth.
    fn before_value(&mut self, depth: usize, offset: usize) -> Result<(), Self::Error>;

    /// Checks an object key before its key event, at the key token's byte
    /// offset.
    fn before_key(&mut self, offset: usize) -> Result<(), Self::Error>;

    /// Checks a number token before its event and before its representation
    /// range is validated. The callback runs synchronously with `token`
    /// borrowed from the input for `'input`. Later syntax, including the
    /// number delimiter and the remainder of the document, has not yet been
    /// validated.
    fn before_number(&mut self, token: &'input str, offset: usize) -> Result<(), Self::Error>;

    /// Receives a lexical event during the synchronous scan; token borrows
    /// remain tied to the input lifetime.
    fn event(&mut self, event: JsonLexicalEvent<'input>) -> Result<(), Self::Error>;
}

/// One callback point dispatched in source scan order.
pub(crate) enum JsonLexicalScanPoint<'input> {
    /// A JSON value is about to be scanned; depth counts the root as one.
    BeforeValue {
        /// Root-inclusive value depth; the document root is depth one.
        depth: usize,
        /// Zero-based byte offset in the original input.
        offset: usize,
    },
    /// An object key is about to be scanned.
    BeforeKey {
        /// Zero-based byte offset of the key token in the original input.
        offset: usize,
    },
    /// A number token is available before representation-range validation.
    BeforeNumber {
        /// Number spelling borrowed from the input for the synchronous
        /// callback.
        token: &'input str,
        /// Zero-based byte offset of the number token in the original input.
        offset: usize,
    },
    /// A token event has been recognized.
    Event(
        /// Event payload, with token borrows tied to the input lifetime.
        JsonLexicalEvent<'input>,
    ),
}
