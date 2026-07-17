// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the message fixture used by decoder tests.

use serde::Deserialize;

/// Typed text payload used by normalization tests.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct Message {
    /// Stores normalized message text.
    pub(crate) text: String,
}
