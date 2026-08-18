// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the location of a JSON tree node within its parent.

/// Identifies a materialized JSON node without exposing a domain path model.
///
/// # Examples
///
/// ```
/// use qubit_json::value::traverse::JsonTreeLocation;
///
/// let location = JsonTreeLocation::ArrayElement { index: 2 };
/// assert_eq!(location, JsonTreeLocation::ArrayElement { index: 2 });
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonTreeLocation<'a> {
    /// The top-level JSON value.
    Root,
    /// An array element identified by its zero-based index.
    ArrayElement {
        /// Zero-based position in the parent array.
        index: usize,
    },
    /// An object value identified by its associated object key.
    ObjectValue {
        /// Key associated with the value in the parent object.
        key: &'a str,
    },
}
