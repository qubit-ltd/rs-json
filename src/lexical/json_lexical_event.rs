// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Synchronous events emitted by the shared JSON lexical scanner.

/// One validated lexical token or container boundary.
///
/// Token offsets are byte offsets into the borrowed input. String and key
/// tokens include both quote bytes; number tokens contain the original bytes.
/// Token slices remain valid for the duration of the scanner callback.
pub(crate) enum JsonLexicalEvent<'input> {
    /// A validated `null` literal.
    Null {
        /// Byte offset where the literal begins in the input.
        offset: usize,
    },
    /// A validated boolean literal.
    Boolean {
        /// Parsed boolean value.
        value: bool,
        /// Byte offset where the literal begins in the input.
        offset: usize,
    },
    /// A validated number with its original token bytes.
    Number {
        /// Original number token bytes borrowed from the input.
        token: &'input [u8],
        /// Byte offset where the token begins in the input.
        offset: usize,
    },
    /// A validated string with its quoted token bytes.
    String {
        /// Original quoted string token bytes borrowed from the input.
        token: &'input [u8],
        /// Byte offset where the token begins in the input.
        offset: usize,
    },
    /// A validated object key with its quoted token bytes.
    Key {
        /// Original quoted object-key token bytes borrowed from the input.
        token: &'input [u8],
        /// Byte offset where the key token begins in the input.
        offset: usize,
    },
    /// The opening byte of an array.
    BeginArray {
        /// Byte offset of the opening bracket in the input.
        offset: usize,
    },
    /// The closing byte of an array.
    EndArray {
        /// Byte offset of the closing bracket in the input.
        offset: usize,
    },
    /// The opening byte of an object.
    BeginObject {
        /// Byte offset of the opening brace in the input.
        offset: usize,
    },
    /// The closing byte of an object.
    EndObject {
        /// Byte offset of the closing brace in the input.
        offset: usize,
    },
}
