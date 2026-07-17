// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the typed payload used by the decoder fuzz target.

use serde::Deserialize;

/// Minimal typed payload used to exercise typed decoder entry points.
#[derive(Deserialize)]
pub(crate) struct FuzzRecord;
