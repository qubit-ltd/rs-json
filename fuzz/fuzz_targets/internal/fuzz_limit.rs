// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared two-byte fuzz limit mapping.

/// Largest generated resource limit.
pub(crate) const MAX_LIMIT: usize = 4 * 1024;

/// Maps two fuzz bytes to the inclusive range `1..=MAX_LIMIT`.
pub(crate) fn limit(data: &[u8], offset: usize) -> usize {
    let low = data.get(offset).copied().unwrap_or_default();
    let high = data.get(offset + 1).copied().unwrap_or_default();
    let raw = u16::from_le_bytes([low, high]);
    1 + usize::from(raw) % MAX_LIMIT
}
