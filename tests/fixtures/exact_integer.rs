// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the exact integer fixture used by decoder tests.

use serde::Deserialize;

/// Payload used to verify lossless `u128` deserialization.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct ExactInteger {
    /// Stores the exact integer value.
    pub(crate) value: u128,
}
