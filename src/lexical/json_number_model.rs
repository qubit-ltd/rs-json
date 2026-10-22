// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Internal numeric range contracts for JSON lexical admission.

/// Selects the representable number range used during JSON admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsonNumberModel {
    /// Preserve the ordinary JSON decoder's signed and unsigned integer limits.
    Integer64,
    /// Interpret every JSON number using finite IEEE 754 binary64 semantics.
    #[cfg(feature = "jcs")]
    Binary64,
}
