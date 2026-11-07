// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines context supplied with one JSON tree callback.

use super::JsonTreeLocation;

/// Describes the root-inclusive depth and parent location of a JSON node.
///
/// # Type Parameters
///
/// * `'a` - Lifetime of an object key borrowed from the traversed JSON tree.
///
/// # Examples
///
/// ```
/// use qubit_json::value::traverse::JsonTreeContext;
/// use qubit_json::value::traverse::JsonTreeLocation;
///
/// let key = "items";
/// let context = JsonTreeContext {
///     depth: 2,
///     location: JsonTreeLocation::ObjectValue { key },
/// };
/// assert_eq!(context.depth, 2);
/// assert_eq!(context.location, JsonTreeLocation::ObjectValue { key });
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JsonTreeContext<'a> {
    /// Root-inclusive depth; the root is always at depth one.
    pub depth: usize,
    /// Root marker or location of this node in its immediate parent.
    pub location: JsonTreeLocation<'a>,
}
