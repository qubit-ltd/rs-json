// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Metadata retained across strict admission and Serde materialization.

use crate::decode::DiagnosticPolicy;

/// Describes the public input boundary used for one decode attempt.
#[derive(Clone, Copy)]
pub(in crate::decode) struct DecodeMetadata {
    /// Original input length observed at the public boundary.
    pub(in crate::decode) raw_input_bytes: usize,
    /// Normalized text length when normalization completed.
    pub(in crate::decode) normalized_input_bytes: Option<usize>,
    /// Policy controlling input-derived diagnostic retention.
    pub(in crate::decode) diagnostic_policy: DiagnosticPolicy,
}
