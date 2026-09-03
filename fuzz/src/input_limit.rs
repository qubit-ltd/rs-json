// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Shared input boundary for all JSON fuzz targets.

/// Maximum complete input size accepted by a JSON fuzz target.
///
/// The fuzz workflows configure libFuzzer with the same maximum length. This
/// defensive check preserves the full-input contract when a target is run
/// outside those workflows.
pub const MAX_FUZZ_INPUT_BYTES: usize = 4_096;

/// Returns the complete fuzz input when it satisfies the shared boundary.
///
/// # Returns
///
/// Returns Some(input) when the input length is at most
/// [`MAX_FUZZ_INPUT_BYTES`]; returns None for oversized inputs rather than
/// silently dropping an unobserved suffix.
#[must_use]
pub fn bounded_input(input: &[u8]) -> Option<&[u8]> {
    (input.len() <= MAX_FUZZ_INPUT_BYTES).then_some(input)
}
