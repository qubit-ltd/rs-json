// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Admission ordering for validated JSON decoding.

/// Selects how validated decoding admits JSON tokens before checking what
/// follows them.
///
/// `Immediate` preserves the existing admission order. `TokenFirst` is an
/// opt-in policy for validating a complete token before reporting a following
/// delimiter error.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum JsonValidatedAdmissionPolicy {
    /// Uses the existing immediate syntax-checking order.
    #[default]
    Immediate,
    /// Admits a complete token before validating its following delimiter.
    TokenFirst,
}
