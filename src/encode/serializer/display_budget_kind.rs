// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Budget categories used by strict display-based JSON encoding.
// qubit-style: allow source-test-pair

/// Resource checked while a `Display` implementation emits text chunks.
#[derive(Clone, Copy)]
pub(super) enum DisplayBudgetKind {
    /// Ordinary JSON string payload.
    String,

    /// JSON object key text.
    Key,

    /// Raw JSON source, bounded by the complete output limit while collected.
    RawOutput,
}
