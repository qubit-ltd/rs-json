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
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use qubit_json::lenient::ErrorPrivacyPolicy;
///
/// fn configured_policy() -> ErrorPrivacyPolicy {
///     ErrorPrivacyPolicy::Redacted
/// }
///
/// configured_policy();
/// ```
#[must_use]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorPrivacyPolicy {
    /// Removes input-derived serde messages and sources from decoding errors.
    #[default]
    Redacted,
    /// Retains complete serde messages and sources for diagnostic use.
    Detailed,
}
