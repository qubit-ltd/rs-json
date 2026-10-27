// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Internal error categories for validated lexical consumption.

use crate::decode::JsonDecodeContext;
use crate::decode::JsonNumberTokenError;
use crate::decode::JsonValidatedLimit;
use crate::value::internal::JsonValueMaterializationError;

/// Internal failures produced while consuming validated lexical events.
///
/// Keeping policy, exact-number, admission-limit, and materialization failures
/// distinct lets the engine preserve their existing public error mapping.
pub(super) enum ValidatedConsumerError<E> {
    /// The caller policy rejected a number token.
    Policy {
        /// Error returned by the policy callback.
        source: E,
        /// Byte offset of the number token.
        offset: usize,
    },
    /// An exact-safe-integer request could not represent the token safely.
    Exact {
        /// Reason the token is outside the exact integer contract.
        source: JsonNumberTokenError,
        /// Byte offset of the number token.
        offset: usize,
    },
    /// A validated decode-specific limit was exceeded.
    Limit {
        /// The configured limit that was exceeded.
        limit: JsonValidatedLimit,
        /// Configured maximum.
        maximum: usize,
        /// Measured value at rejection.
        observed: usize,
        /// Byte offset associated with the rejected event.
        offset: usize,
    },
    /// The guarded value builder rejected an event.
    Materialization {
        /// Builder failure, which also clears its partial value.
        source: JsonValueMaterializationError,
        /// Decode context used by the engine error adapter.
        context: JsonDecodeContext,
    },
}
