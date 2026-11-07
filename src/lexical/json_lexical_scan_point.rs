// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::json_lexical_event::JsonLexicalEvent;

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
