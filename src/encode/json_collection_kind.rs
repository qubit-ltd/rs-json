// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines JSON collection kinds used by value-encoding errors.

/// JSON collection whose item count could not be represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonCollectionKind {
    /// A JSON array.
    Array,
    /// A JSON object.
    Object,
}
