// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Assertions shared by private Value protocol tests.

use qubit_json::encode::JsonSerializationError;
use qubit_json::encode::JsonSerializerStateError;

/// Asserts the stable Number protocol error kind.
pub(super) fn assert_number_protocol_error(error: &JsonSerializationError) {
    assert_eq!(
        error.serializer_state_error(),
        Some(JsonSerializerStateError::InvalidNumberProtocol),
    );
}
