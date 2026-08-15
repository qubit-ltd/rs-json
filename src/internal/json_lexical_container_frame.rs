// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Continuation state for non-recursive JSON lexical scanning.

/// Continuation for one JSON container being scanned iteratively.
pub(super) enum JsonLexicalContainerFrame {
    /// An array ready for its first or next value.
    ArrayValue {
        /// Root-inclusive depth of the array.
        depth: usize,
        /// Items already admitted in this array.
        items: usize,
    },
    /// An array waiting for a comma or closing bracket.
    ArrayDelimiter {
        /// Root-inclusive depth of the array.
        depth: usize,
        /// Items already admitted in this array.
        items: usize,
    },
    /// An object ready for its first or next key.
    ObjectKey {
        /// Root-inclusive depth of the object.
        depth: usize,
        /// Entries already admitted in this object.
        entries: usize,
    },
    /// An object waiting for a comma or closing brace.
    ObjectDelimiter {
        /// Root-inclusive depth of the object.
        depth: usize,
        /// Entries already admitted in this object.
        entries: usize,
    },
}
