// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Assertions shared by private serde_json protocol tests.

use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncodeError;
use qubit_json::encode::JsonSerializerStateError;

/// Asserts the precise private protocol state surfaced by a public error.
pub(super) fn assert_private_state_error(
    error: &JsonEncodeError<JsonResource>,
    expected: JsonSerializerStateError,
) {
    assert_eq!(
        error
            .serialization_error()
            .and_then(|error| error.serializer_state_error()),
        Some(expected),
    );
}
