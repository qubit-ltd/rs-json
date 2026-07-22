// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the private size-limit discriminator for decoding errors.

/// Identifies the size limit that rejected JSON input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::error) enum JsonInputSizeLimit {
    /// Stores the configured raw input byte limit.
    Raw(usize),
    /// Stores the configured normalized JSON byte limit.
    Normalized(usize),
}
