// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines violations of the Serde compound-serialization protocol.

/// Invalid state produced by a malformed hand-written Serde implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonSerializerStateError {
    /// A compound callback was used with the wrong compound state.
    UnexpectedCompound,
    /// A split-map key was submitted before the previous key received a value.
    MapKeyAlreadyPending,
    /// A split-map value was submitted without a preceding key.
    MapValueWithoutKey,
    /// A split map ended while one key was still waiting for its value.
    MapEndedWithPendingKey,
    /// The private serde_json RawValue protocol had an invalid shape.
    InvalidRawValueProtocol,
}
