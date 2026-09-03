// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the privacy policy for JSON decoding error diagnostics.

/// Controls whether JSON decoding errors retain input-derived serde details.
///
/// Redacted diagnostics are safe by default. Detailed diagnostics are intended
/// only for controlled environments because they may contain input values.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::DiagnosticPolicy;
///
/// let policy = DiagnosticPolicy::Redacted;
/// assert_ne!(policy, DiagnosticPolicy::Detailed);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiagnosticPolicy {
    /// Removes input-derived serde messages and sources from decoding errors.
    #[default]
    Redacted,
    /// Retains complete serde messages and sources for diagnostic use.
    Detailed,
}
