// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the duplicate-field fixture used by decoder tests.

use serde::Deserialize;

/// Payload used to verify serde duplicate-field rejection.
#[derive(Debug, Deserialize)]
pub(crate) struct SingleValue {
    /// Stores the sole expected value.
    #[serde(rename = "value")]
    _value: u8,
}
