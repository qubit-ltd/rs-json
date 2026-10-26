// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Structured reasons for failures while materializing unique-key JSON.

/// Describes why an admitted unique-key document could not be materialized.
///
/// The reason is available independently of diagnostic source retention and
/// never contains input-derived text.
#[non_exhaustive]
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonMaterializationErrorReason {
    /// A decoded object key appeared more than once.
    DuplicateKey,
    /// The fixed intermediate-tree container limit was exceeded.
    DepthLimit {
        /// Maximum number of nested containers accepted during construction.
        limit: usize,
    },
    /// An admitted number could not be represented by the intermediate tree.
    InvalidNumber,
}
