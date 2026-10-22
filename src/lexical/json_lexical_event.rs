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
    Null { offset: usize },
    /// A validated boolean literal.
    Boolean { value: bool, offset: usize },
    /// A validated number with its original token bytes.
    Number { token: &'input [u8], offset: usize },
    /// A validated string with its quoted token bytes.
    String { token: &'input [u8], offset: usize },
    /// A validated object key with its quoted token bytes.
    Key { token: &'input [u8], offset: usize },
    /// The opening byte of an array.
    BeginArray { offset: usize },
    /// The closing byte of an array.
    EndArray { offset: usize },
    /// The opening byte of an object.
    BeginObject { offset: usize },
    /// The closing byte of an object.
    EndObject { offset: usize },
}
