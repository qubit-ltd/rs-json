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
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JsonTreeContext<'a> {
    /// Root-inclusive depth; the root is always at depth one.
    pub depth: usize,
    /// Location of this node in its immediate parent.
    pub location: JsonTreeLocation<'a>,
}
