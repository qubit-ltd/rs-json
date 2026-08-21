// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines budget semantics for serde_json private text payloads.

/// Budget semantics for a serde_json private string payload.
#[derive(Clone, Copy)]
pub(in crate::encode::serializer) enum PrivateTextKind {
    /// Arbitrary-precision number text.
    Number { depth: usize },

    /// Raw JSON fragment rooted at the specified final depth.
    RawValue { depth: usize },
}
