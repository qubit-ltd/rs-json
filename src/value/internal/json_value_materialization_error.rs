// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Non-sensitive failures produced while constructing a lexical JSON value.

/// A materialization failure stores only its kind, location, and fixed limit.
#[must_use]
#[derive(Debug)]
pub(crate) enum JsonValueMaterializationError {
    /// A decoded object key was already present in the current object.
    DuplicateKey {
        /// Byte offset of the second key token's opening quote.
        offset: usize,
    },
    /// A container would exceed the fixed construction depth.
    DepthLimit {
        /// Byte offset of the first over-limit container token.
        offset: usize,
        /// Maximum number of nested containers accepted by the builder.
        limit: usize,
    },
    /// An admitted number could not be represented under the selected model.
    InvalidNumber { offset: usize },
}
