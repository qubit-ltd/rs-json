// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Errors reported while rendering a JSON value as JCS.

use serde_json::Error;

use crate::encode::JcsWorkingMemoryError;

/// Describes failures encountered while rendering a JSON value as JCS.
#[must_use]
#[derive(Debug)]
pub(in crate::encode) enum JcsRendererError {
    /// The renderer exceeded its configured working-memory budget.
    WorkingMemory(JcsWorkingMemoryError),
    /// Canonical JSON serialization or output writing failed.
    Serialize(Error),
    /// An auxiliary allocation could not be completed.
    Allocation,
}
