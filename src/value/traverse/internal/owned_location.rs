// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines owned visitor locations used during mutable traversal.

use super::super::JsonTreeContext;
use super::super::JsonTreeLocation;

/// Owns the location information needed while mutable traversal advances.
#[derive(Clone)]
pub(in crate::value::traverse) enum OwnedLocation {
    Root,
    ArrayElement(usize),
    ObjectValue(String),
}

impl OwnedLocation {
    /// Borrows this owned location for a visitor context.
    #[inline(always)]
    pub(in crate::value::traverse) fn context(&self, depth: usize) -> JsonTreeContext<'_> {
        let location = match self {
            Self::Root => JsonTreeLocation::Root,
            Self::ArrayElement(index) => JsonTreeLocation::ArrayElement { index: *index },
            Self::ObjectValue(key) => JsonTreeLocation::ObjectValue { key },
        };
        JsonTreeContext { depth, location }
    }
    /// Returns the object key that must be charged before child admission.
    #[must_use]
    #[inline(always)]
    pub(in crate::value::traverse) fn key(&self) -> Option<&str> {
        match self {
            Self::ObjectValue(key) => Some(key),
            Self::Root | Self::ArrayElement(_) => None,
        }
    }
}
