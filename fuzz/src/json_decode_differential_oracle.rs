// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Semantic relations checked by the JSON decode differential fuzz target.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use serde_json::Value;

use crate::json_number_contract::numbers_fit_contract;

/// Conservative upper bound for inputs compared directly with serde_json.
///
/// Counting every opening delimiter, including delimiters inside strings,
/// intentionally under-approximates the set of inputs that use the reference
/// parser. Inputs beyond this bound still exercise decoder/validator relations
/// without conflating lexical admission with serde_json's recursion limit.
const MAX_REFERENCE_OPENING_DELIMITERS: usize = 64;

/// Asserts the decode and validation contracts for one bounded fuzz input.
///
/// Successful value materialization must imply successful lexical admission.
/// A lexically admitted document may still fail materialization because the
/// target type or serde_json's recursion limit rejects it; such failures must
/// be classified as JsonDecodeErrorKind::Deserialize. Direct equivalence with
/// serde_json is checked only for conservatively shallow inputs.
///
/// # Panics
///
/// Panics when decoder and validator results violate their documented
/// relationship, or when a shallow input disagrees with serde_json and the
/// public number contract.
pub fn assert_decode_contract(input: &[u8]) {
    let decode_session =
        JsonDecodeSession::from_limits(JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build());
    let decoded = JsonDecoder::new(decode_session).decode_utf8::<Value>(input);
    let validation_session =
        JsonDecodeSession::from_limits(JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build());
    let validated = JsonDecoder::new(validation_session).validate_utf8(input);

    assert!(
        decoded.is_err() || validated.is_ok(),
        "successful materialization must imply successful validation",
    );
    if validated.is_ok()
        && let Err(error) = &decoded
    {
        assert_eq!(
            error.kind(),
            JsonDecodeErrorKind::Deserialize,
            "a lexically admitted document may fail only during materialization",
        );
    }

    if is_within_reference_nesting_domain(input) {
        let reference_admitted = serde_json::from_slice::<Value>(input).is_ok() && numbers_fit_contract(input);
        assert_eq!(
            decoded.is_ok(),
            reference_admitted,
            "shallow strict materialization must match the reference contract",
        );
        assert_eq!(
            validated.is_ok(),
            reference_admitted,
            "shallow strict validation must match the reference contract",
        );
    }
}

/// Reports whether an input is safely below the reference parser's recursion
/// domain.
///
/// The raw delimiter count is deliberately conservative: delimiters in strings
/// can only skip a valid reference comparison, never admit a deep document to
/// it.
#[must_use]
fn is_within_reference_nesting_domain(input: &[u8]) -> bool {
    input
        .iter()
        .filter(|byte| matches!(**byte, b'[' | b'{'))
        .take(MAX_REFERENCE_OPENING_DELIMITERS)
        .count()
        < MAX_REFERENCE_OPENING_DELIMITERS
}
