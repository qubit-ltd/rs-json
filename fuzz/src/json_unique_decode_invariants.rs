// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Assertions shared by deterministic tests and the unique-key JSON fuzz
//! target.

use std::error::Error as StdError;

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonMaterializationErrorReason;
use qubit_json::value::DuplicateKeyRejectingJsonValue;
use qubit_json::value::DuplicateKeyRejectingJsonValueSeed;
use serde::de::DeserializeSeed;
use serde_json::Value;

use crate::json_number_contract::numbers_fit_contract;
use crate::unique_decode_reference::parse_unique_literal_value;

const MAX_REFERENCE_OPENING_DELIMITERS: usize = 64;
const REDACTED_DUPLICATE_FIXTURE: &[u8] =
    b"{\"unique-decode-sensitive-marker-7f3a\":1,\"unique-decode-sensitive-marker-7f3a\":2}";
const SENSITIVE_MARKER: &str = "unique-decode-sensitive-marker-7f3a";

/// Checks unique-key decode entry points, independent reference semantics, and
/// accounting invariants.
pub fn assert_unique_decode_invariants(input: &[u8], max_nodes: usize) {
    let baseline_limits = decode_limits(input, usize::MAX);
    let mut baseline = JsonDecoder::with_limits(baseline_limits);
    let baseline_result = baseline.decode_unique_utf8::<DuplicateKeyRejectingJsonValue>(input);
    let baseline_nodes = baseline.session().value_budget().used_nodes();
    assert!(baseline_nodes.is_some(), "baseline must measure source nodes");
    if baseline_result.is_ok() {
        assert!(
            baseline_nodes.is_some_and(|nodes| nodes > 0),
            "a successful baseline must expose source node usage",
        );
    }
    assert_eq!(
        baseline.session().input_budget().map(|budget| budget.used()),
        Some(input.len()),
        "baseline must account for the complete input",
    );

    if let Ok(text) = std::str::from_utf8(input) {
        let mut decoder = JsonDecoder::with_limits(decode_limits(input, usize::MAX));
        let string_result = decoder.decode_unique_str::<DuplicateKeyRejectingJsonValue>(text);
        assert_results_equivalent(&baseline_result, &string_result);
        assert_failure_rollback(&string_result, decoder.session().value_budget().used_nodes());
        assert_input_accounted(&decoder, input.len());
    }

    let mut seed_decoder = JsonDecoder::with_limits(decode_limits(input, usize::MAX));
    let seed_result = seed_decoder.decode_unique_seed_utf8(DuplicateKeyRejectingJsonValueSeed::new(), input);
    assert_results_equivalent(&baseline_result, &seed_result);
    assert_failure_rollback(&seed_result, seed_decoder.session().value_budget().used_nodes());
    assert_input_accounted(&seed_decoder, input.len());

    if is_within_reference_domain(input) {
        let reference = parse_unique_literal_value(input);
        let reference_admitted = reference.as_ref().is_ok_and(|_| numbers_fit_contract(input));
        assert_eq!(
            baseline_result.is_ok(),
            reference_admitted,
            "shallow unique decode must match the independent literal-key reference",
        );
        if reference_admitted {
            let expected = reference.expect("admitted reference input must parse successfully");
            let mut comparison_decoder = JsonDecoder::with_limits(decode_limits(input, usize::MAX));
            let actual = comparison_decoder
                .decode_unique_utf8::<DuplicateKeyRejectingJsonValue>(input)
                .expect("reference-accepted shallow input must decode");
            assert_values_equal(&actual.into_inner(), &expected);
        }
    }

    assert_failure_rollback(&baseline_result, baseline.session().value_budget().used_nodes());
    assert_redacted_duplicate_diagnostics(input);
    assert_equal_source_node_budget(input, &baseline_result, baseline_nodes);
    assert_node_budget_relation(input, max_nodes, &baseline_result, baseline_nodes);
    assert_failed_seed_session_reuse();
}

fn decode_limits(input: &[u8], max_nodes: usize) -> JsonDecodeLimits<JsonResource, usize> {
    JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(input.len())
        .max_nodes(max_nodes)
        .build()
}

fn assert_input_accounted(decoder: &JsonDecoder<'_, JsonResource, usize>, input_bytes: usize) {
    assert_eq!(
        decoder.session().input_budget().map(|budget| budget.used()),
        Some(input_bytes),
        "a failed decode must retain its accepted input-byte charge",
    );
}

fn is_within_reference_domain(input: &[u8]) -> bool {
    input
        .iter()
        .filter(|byte| matches!(**byte, b'{' | b'['))
        .take(MAX_REFERENCE_OPENING_DELIMITERS)
        .count()
        < MAX_REFERENCE_OPENING_DELIMITERS
}

fn error_signature<T>(
    result: &Result<T, JsonDecodeError>,
) -> Option<(
    JsonDecodeErrorKind,
    Option<JsonMaterializationErrorReason>,
    Option<usize>,
    Option<usize>,
)> {
    result.as_ref().err().map(|error| {
        (
            error.kind(),
            error.materialization_error_reason(),
            error.line(),
            error.column(),
        )
    })
}

fn assert_results_equivalent<T>(left: &Result<T, JsonDecodeError>, right: &Result<T, JsonDecodeError>)
where
    T: PartialEq + std::fmt::Debug,
{
    assert_eq!(error_signature(left), error_signature(right));
    assert_eq!(left.as_ref().ok(), right.as_ref().ok());
}

fn assert_values_equal(actual: &Value, expected: &Value) {
    assert_eq!(actual, expected, "unique output must equal the literal reference value");
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(actual.is_i64(), expected.is_i64(), "signed-integer category must be preserved");
            assert_eq!(actual.is_u64(), expected.is_u64(), "unsigned-integer category must be preserved");
            assert_eq!(actual.is_f64(), expected.is_f64(), "number category must be preserved");
            if actual.is_f64() {
                assert_eq!(
                    actual
                        .as_f64()
                        .expect("floating number has an f64 value")
                        .is_sign_negative(),
                    expected
                        .as_f64()
                        .expect("reference float has an f64 value")
                        .is_sign_negative(),
                    "floating-point sign, including negative zero, must be preserved",
                );
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            for (actual, expected) in actual.iter().zip(expected) {
                assert_values_equal(actual, expected);
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            for (key, actual) in actual {
                assert_values_equal(actual, &expected[key]);
            }
        }
        _ => {}
    }
}

fn assert_failure_rollback<T>(result: &Result<T, JsonDecodeError>, used_nodes: Option<usize>) {
    if result.is_err() {
        assert_eq!(
            used_nodes,
            Some(0),
            "failed materialization must not commit value nodes"
        );
    }
}

fn assert_redacted_duplicate_diagnostics(input: &[u8]) {
    if input != REDACTED_DUPLICATE_FIXTURE {
        return;
    }
    let mut decoder = JsonDecoder::with_limits(decode_limits(input, usize::MAX));
    let error = decoder
        .decode_unique_utf8::<DuplicateKeyRejectingJsonValue>(input)
        .expect_err("the fixed sensitive duplicate-key fixture must fail");
    assert_eq!(
        error.materialization_error_reason(),
        Some(JsonMaterializationErrorReason::DuplicateKey)
    );
    assert!(!error.to_string().contains(SENSITIVE_MARKER));
    assert!(!format!("{error:?}").contains(SENSITIVE_MARKER));
    assert!(
        !error
            .source()
            .is_some_and(|source| source.to_string().contains(SENSITIVE_MARKER))
    );
}

fn assert_node_budget_relation(
    input: &[u8],
    max_nodes: usize,
    baseline_result: &Result<DuplicateKeyRejectingJsonValue, JsonDecodeError>,
    baseline_nodes: Option<usize>,
) {
    let mut constrained = JsonDecoder::with_limits(decode_limits(input, max_nodes));
    let constrained_result = constrained.decode_unique_utf8::<DuplicateKeyRejectingJsonValue>(input);
    assert_eq!(
        constrained.session().input_budget().map(|budget| budget.used()),
        Some(input.len()),
        "node-limited decoding must account for the full input",
    );
    match baseline_result {
        Ok(expected) if max_nodes >= baseline_nodes.expect("baseline measures nodes") => {
            let actual = constrained_result.expect("sufficient source-node budget must succeed");
            assert_eq!(actual, *expected);
            assert_eq!(constrained.session().value_budget().used_nodes(), baseline_nodes);
        }
        Ok(_) => {
            let error = constrained_result.expect_err("insufficient source-node budget must reject");
            assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
            assert_eq!(constrained.session().value_budget().used_nodes(), Some(0));
        }
        Err(_) => {
            assert!(
                constrained_result.is_err(),
                "stricter node limits must not admit baseline failures"
            );
            assert_eq!(constrained.session().value_budget().used_nodes(), Some(0));
        }
    }
}

fn assert_equal_source_node_budget(
    input: &[u8],
    baseline_result: &Result<DuplicateKeyRejectingJsonValue, JsonDecodeError>,
    baseline_nodes: Option<usize>,
) {
    let Some(expected_nodes) = baseline_nodes else {
        return;
    };
    if let Ok(expected) = baseline_result {
        let mut exact = JsonDecoder::with_limits(decode_limits(input, expected_nodes));
        let actual = exact
            .decode_unique_utf8::<DuplicateKeyRejectingJsonValue>(input)
            .expect("a budget equal to measured source nodes must succeed");
        assert_eq!(actual, *expected);
        assert_eq!(exact.session().value_budget().used_nodes(), Some(expected_nodes));
        assert_eq!(
            exact.session().input_budget().map(|budget| budget.used()),
            Some(input.len()),
        );
        if let Some(below_nodes) = expected_nodes.checked_sub(1) {
            let mut below = JsonDecoder::with_limits(decode_limits(input, below_nodes));
            let error = below
                .decode_unique_utf8::<DuplicateKeyRejectingJsonValue>(input)
                .expect_err("a budget below measured source nodes must reject");
            assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
            assert_eq!(below.session().value_budget().used_nodes(), Some(0));
            assert_eq!(
                below.session().input_budget().map(|budget| budget.used()),
                Some(input.len()),
            );
        }
    }
}

struct FailingSeed;

impl<'de> DeserializeSeed<'de> for FailingSeed {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, _deserializer: D) -> Result<(), D::Error> {
        Err(serde::de::Error::custom("intentional seed failure"))
    }
}

fn assert_failed_seed_session_reuse() {
    let first = b"true";
    let second = b"null";
    let total_input = first.len() + second.len();
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(total_input)
        .max_nodes(usize::MAX)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let failed = decoder.decode_unique_seed_utf8(FailingSeed, first);
    assert_eq!(
        failed.expect_err("the injected seed must fail").kind(),
        JsonDecodeErrorKind::Deserialize
    );
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(
        decoder.session().input_budget().map(|budget| budget.used()),
        Some(first.len())
    );
    let before_second_nodes = decoder.session().value_budget().used_nodes();
    let value = decoder
        .decode_unique_utf8::<Value>(second)
        .expect("a failed seed must leave the session reusable");
    assert_eq!(value, Value::Null);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
    assert_eq!(before_second_nodes, Some(0));
    assert_eq!(
        decoder.session().input_budget().map(|budget| budget.used()),
        Some(total_input)
    );

    let one_input = second.len();
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(one_input)
        .max_nodes(usize::MAX)
        .build();
    let mut exhausted = JsonDecoder::with_limits(limits);
    exhausted
        .decode_unique_utf8::<Value>(second)
        .expect("the first input exactly fits the session budget");
    let first_nodes = exhausted.session().value_budget().used_nodes();
    assert_eq!(first_nodes, Some(1));
    let rejection = exhausted
        .decode_unique_utf8::<Value>(second)
        .expect_err("the next input must use the remaining cumulative budget");
    assert_eq!(rejection.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(exhausted.session().value_budget().used_nodes(), first_nodes);
    assert_eq!(
        exhausted.session().input_budget().map(|budget| budget.used()),
        Some(one_input),
        "rejected input must not reset or exceed cumulative input accounting",
    );
}
