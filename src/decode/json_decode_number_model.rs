// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Number representations accepted by JSON decoding.

/// Controls lexical admission of syntactically valid JSON number tokens.
///
/// Typed decoding still lets Serde construct the requested target type, so its
/// numeric representation can depend on that type and the active Serde
/// features. Validated decoding constructs its owned value according to this
/// model and the selected number policy.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum JsonDecodeNumberModel {
    /// Accept signed and unsigned 64-bit integers and finite floating values.
    #[default]
    Integer64,
    /// Also admit out-of-range integer tokens when they convert to finite
    /// `f64` values.
    ///
    /// Validated decoding with `JsonNumberPolicy::Preserve` materializes these
    /// tokens as finite `f64`; typed decoding leaves their representation to
    /// Serde for the requested target type.
    Integer64WithFiniteFallback,
}
