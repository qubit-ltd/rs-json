// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the signedness of a rejected wide JSON integer.

/// Signedness retained by an integer-range encoding failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonIntegerSignedness {
    /// The source was serialized through a signed integer entry point.
    Signed,
    /// The source was serialized through an unsigned integer entry point.
    Unsigned,
}
