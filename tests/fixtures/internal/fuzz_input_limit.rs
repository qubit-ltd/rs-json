// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the decoder fuzz harness input boundary.

/// Maximum byte length exercised by the decoder fuzz harness.
pub(crate) const MAX_FUZZ_INPUT_BYTES: usize = 4_096;

/// Reports whether an input is within the decoder fuzz harness boundary.
///
/// # Parameters
///
/// * `input` - Arbitrary byte input considered for fuzz decoding.
///
/// # Returns
///
/// `true` when the input length is at most [`MAX_FUZZ_INPUT_BYTES`].
#[must_use]
pub(crate) fn is_fuzz_input_within_limit(input: &[u8]) -> bool {
    input.len() <= MAX_FUZZ_INPUT_BYTES
}
