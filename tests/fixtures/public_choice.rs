// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the enum-shaped fixture used by error privacy tests.

use serde::Deserialize;

/// Public enum-shaped payload used to verify error redaction.
#[derive(Debug, Deserialize)]
pub(crate) enum PublicChoice {
    /// Represents the only accepted public choice.
    Allowed,
}
