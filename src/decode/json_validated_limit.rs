// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Additional validated decoding limits.

/// Identifies an additional per-document decode limit.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonValidatedLimit {
    /// Value entries plus decoded object-key entries.
    ValueAndKeyNodes,
    /// Source bytes inside one encoded string token, excluding quotes.
    EncodedStringBytes,
}
