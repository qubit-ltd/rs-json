// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the privacy policy for JSON decoding error diagnostics.

/// Controls whether library-generated JSON decoding diagnostics retain
/// input-derived details.
///
/// Redacted diagnostics are safe by default. Detailed diagnostics are intended
/// only for controlled environments because they may contain input values.
/// This policy does not redact errors returned by caller-provided validated
/// number policies. [`super::JsonValidatedDecodeError::Policy`] preserves that
/// error as its source, although the outer error's `Display` and `Debug` omit
/// it. Callers are responsible for the contents and logging of their policy
/// errors. See [`JsonValidatedDecodeError`](super::JsonValidatedDecodeError)
/// for a complete example.
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
    /// Retains only stable classifications and source coordinates in
    /// library-generated diagnostics.
    ///
    /// Redacted library diagnostics never retain unexpected bytes,
    /// token/key/value text, or parser and Serde sources. Caller-provided
    /// validated policy errors are not controlled by this variant.
    #[default]
    Redacted,
    /// Retains complete serde messages and sources for diagnostic use.
    Detailed,
}
