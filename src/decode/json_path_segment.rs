// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Typed location within a JSON document.

/// Identifies an object key or array index in a JSON value path.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JsonPathSegment {
    /// An object key after JSON escape decoding.
    Key(String),
    /// An array element index.
    Index(usize),
}
