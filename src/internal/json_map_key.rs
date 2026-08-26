// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared canonical text conversions for JSON object keys.

/// Provides conversions shared by strict JSON map-key serializers.
pub(crate) enum JsonMapKey {
    // empty
}

impl JsonMapKey {
    /// Converts a full-range signed integer key to canonical decimal text.
    #[inline]
    #[must_use]
    pub(crate) fn signed_wide(value: i128) -> String {
        value.to_string()
    }

    /// Converts a full-range unsigned integer key to canonical decimal text.
    #[inline]
    #[must_use]
    pub(crate) fn unsigned_wide(value: u128) -> String {
        value.to_string()
    }
}
