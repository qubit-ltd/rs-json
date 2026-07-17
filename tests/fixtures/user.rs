// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the user fixture used by decoder tests.

use serde::Deserialize;

/// Typed user payload used by decoder tests.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct User {
    /// Stores the user name.
    pub(crate) name: String,
    /// Stores the user age.
    pub(crate) age: u8,
}
