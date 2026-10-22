// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Phases of a serde_json private compound protocol.

/// Current phase of one private protocol payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsonPrivateProtocolPhase {
    /// No payload field has started.
    Pending,
    /// The payload field callback is running.
    InFlight,
    /// Exactly one payload completed successfully.
    Complete,
    /// A protocol violation or payload failure occurred.
    Failed,
}
