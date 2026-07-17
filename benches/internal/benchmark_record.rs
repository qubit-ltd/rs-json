// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the typed payload used by decoder benchmarks.

use serde::Deserialize;

/// Typed value used by constrained-decoder benchmarks.
#[derive(Deserialize)]
pub(crate) struct BenchmarkRecord {
    /// Identifies the benchmark record.
    pub(crate) id: u64,
    /// Stores a representative short text value.
    pub(crate) text: String,
}
