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
/// # Examples
///
/// ```
/// use qubit_json::value::traverse::{JsonTreeContext, JsonTreeLocation};
///
/// let context = JsonTreeContext {
///     depth: 1,
///     location: JsonTreeLocation::Root,
/// };
/// assert_eq!(context.depth, 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JsonTreeContext<'a> {
    /// Root-inclusive depth; the root is always at depth one.
    pub depth: usize,
    /// Location of this node in its immediate parent.
    pub location: JsonTreeLocation<'a>,
}
