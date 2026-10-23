// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared typed payload for the decoder fuzz target and corpus regression.

use serde::Deserialize;

/// Representative object decoded by the typed fuzz path.
#[derive(Debug, Deserialize, Eq, PartialEq)]
pub struct FuzzRecord {
    /// Full-range unsigned record identifier.
    pub id: u64,
    /// Owned label decoded from the JSON string.
    pub label: String,
    /// Whether the record is enabled.
    pub enabled: bool,
    /// Ordered tags retained from the input array.
    pub tags: Vec<String>,
}
