// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Number representations accepted by JSON decoding.

/// Controls conversion of syntactically valid JSON number tokens.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum JsonDecodeNumberModel {
    /// Accept signed and unsigned 64-bit integers and finite floating values.
    #[default]
    Integer64,
    /// Also represent out-of-range integer tokens as finite `f64` values.
    Integer64WithFiniteFallback,
}
