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
use std::fmt;

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonMaterializationErrorReason;
use qubit_json::decode::JsonNumberPolicy;
use qubit_json::format::JsonPrettyOptions;
use qubit_json::format::format_json_tokens;
use qubit_json::format::write_json_tokens;
use qubit_json::value::DuplicateKeyRejectingJsonValue;
use qubit_json::value::DuplicateKeyRejectingJsonValueSeed;
use serde::de::DeserializeSeed;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde_json::Value;

use crate::json_number_contract::numbers_fit_contract;
use crate::unique_decode_reference::parse_unique_literal_value;

const MAX_REFERENCE_OPENING_DELIMITERS: usize = 64;
const REDACTED_DUPLICATE_FIXTURE: &[u8] =
    b"{\"unique-decode-sensitive-marker-7f3a\":1,\"unique-decode-sensitive-marker-7f3a\":2}";
const SENSITIVE_MARKER: &str = "unique-decode-sensitive-marker-7f3a";
type ValidatedLimitResult = (
    Result<
        qubit_json::value::SafeJsonValue,
        qubit_json::decode::JsonValidatedDecodeError<std::convert::Infallible, JsonResource, usize>,
    >,
    JsonDecoder<'static, JsonResource, usize>,
);
type DecodeErrorSignature = (
    JsonDecodeErrorKind,
    Option<JsonMaterializationErrorReason>,
    Option<usize>,
    Option<usize>,
);

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
    assert_protected_decode_invariants(input);
    assert_token_format_invariants(input);
}

/// Checks protected and validated decode agreement, plus token-preserving
/// formatter invariants for a complete bounded input.
pub fn assert_protected_decode_invariants(input: &[u8]) {
    if input.len() > 4096 {
        return;
    }
    let mut protected = JsonDecoder::with_limits(decode_limits(input, usize::MAX));
    let actual = protected.decode_value_utf8(input);
    if let Ok(actual) = actual {
        let options = qubit_json::decode::JsonValidatedDecodeOptions::builder().build();
        let mut validated = JsonDecoder::with_limits(decode_limits(input, usize::MAX));
        let mut events = Vec::new();
        let checked =
            validated.decode_unique_validated_utf8::<std::convert::Infallible, _>(input, &options, |path, token| {
                events.push((path.to_vec(), token.raw().to_owned(), token.offset()));
                Ok(qubit_json::decode::JsonNumberPolicy::Preserve)
            });
        let checked = checked.expect("Preserve validation must accept protected-path successes");
        if is_within_reference_domain(input) {
            assert_values_equal(actual.as_value(), checked.as_value());
            assert_number_events_match_reference(input, &events);
        }
        for (_, raw, offset) in &events {
            assert_eq!(
                input.get(*offset..offset.saturating_add(raw.len())),
                Some(raw.as_bytes())
            );
        }
        if input == br#"[1,{"x":2},3]"# {
            use qubit_json::decode::JsonPathSegment;
            assert_eq!(
                events.iter().map(|event| event.0.clone()).collect::<Vec<_>>(),
                vec![
                    vec![JsonPathSegment::Index(0)],
                    vec![JsonPathSegment::Index(1), JsonPathSegment::Key("x".to_owned())],
                    vec![JsonPathSegment::Index(2)],
                ],
                "number callback paths must follow source array order",
            );
            assert_eq!(
                events.iter().map(|event| event.1.as_str()).collect::<Vec<_>>(),
                ["1", "2", "3"]
            );
        }
        if input == br#"{"a\u0062":[4,5]}"# {
            use qubit_json::decode::JsonPathSegment;
            assert_eq!(
                events.iter().map(|event| event.0.clone()).collect::<Vec<_>>(),
                vec![
                    vec![JsonPathSegment::Key("ab".to_owned()), JsonPathSegment::Index(0)],
                    vec![JsonPathSegment::Key("ab".to_owned()), JsonPathSegment::Index(1)],
                ],
                "escaped object keys must be decoded in callback paths",
            );
            assert_eq!(
                events.iter().map(|event| event.1.as_str()).collect::<Vec<_>>(),
                ["4", "5"]
            );
        }
        assert_exact_integer_oracle(input);
    } else {
        assert_eq!(protected.session().value_budget().used_nodes(), Some(0));
        assert_eq!(
            protected.session().input_budget().map(|budget| budget.used()),
            Some(input.len())
        );
        let options = qubit_json::decode::JsonValidatedDecodeOptions::builder().build();
        let mut validated = JsonDecoder::with_limits(decode_limits(input, usize::MAX));
        let result = validated.decode_unique_validated_utf8::<std::convert::Infallible, _>(input, &options, |_, _| {
            Ok(JsonNumberPolicy::Preserve)
        });
        assert!(
            matches!(result, Err(qubit_json::decode::JsonValidatedDecodeError::Decode { .. })),
            "Preserve validation must report a decode failure for protected-path failures"
        );
        // The two entry points can fail at different stages, so compare only
        // the broad validated error class and each path's transaction state.
        assert_eq!(validated.session().value_budget().used_nodes(), Some(0));
        assert_eq!(
            validated.session().input_budget().map(|budget| budget.used()),
            Some(input.len()),
        );
        let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
            .max_input_bytes(input.len().saturating_add(1))
            .max_nodes(usize::MAX)
            .build();
        let mut reusable = JsonDecoder::with_limits(limits);
        assert!(reusable.decode_value_utf8(input).is_err());
        assert_eq!(reusable.session().value_budget().used_nodes(), Some(0));
        reusable
            .decode_value_str("0")
            .expect("a failed protected decode must leave the session reusable");
        assert_eq!(
            reusable.session().input_budget().map(|budget| budget.used()),
            Some(input.len().saturating_add(1)),
        );
    }
}

/// Compares every validated number callback with a source-ordered Serde
/// reference tree on the bounded shallow-input domain.
fn assert_number_events_match_reference(
    input: &[u8],
    events: &[(Vec<qubit_json::decode::JsonPathSegment>, String, usize)],
) {
    let reference: ReferenceJsonValue =
        serde_json::from_slice(input).expect("protected success must parse as reference JSON");
    let mut expected = Vec::new();
    collect_reference_numbers(&reference, &mut Vec::new(), &mut expected);
    assert_eq!(
        events.len(),
        expected.len(),
        "validated callbacks must report every source number exactly once",
    );
    for ((actual_path, raw, offset), (expected_path, expected_number)) in events.iter().zip(expected) {
        assert_eq!(
            actual_path, &expected_path,
            "validated number callback path and order must match the independent reference",
        );
        let end = offset.saturating_add(raw.len());
        assert_eq!(input.get(*offset..end), Some(raw.as_bytes()));
        let actual_number: serde_json::Number =
            serde_json::from_str(raw).expect("number callback raw text must be one JSON number");
        assert_eq!(
            actual_number, expected_number,
            "number callback at {actual_path:?} must match its reference value",
        );
    }
}

enum ReferenceJsonValue {
    Null,
    Bool,
    Number(serde_json::Number),
    String,
    Array(Vec<Self>),
    Object(Vec<(String, Self)>),
}

impl<'de> serde::Deserialize<'de> for ReferenceJsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ReferenceVisitor;

        impl<'de> Visitor<'de> for ReferenceVisitor {
            type Value = ReferenceJsonValue;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON value for the independent number-path oracle")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(ReferenceJsonValue::Null)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(ReferenceJsonValue::Null)
            }

            fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
                Ok(ReferenceJsonValue::Bool)
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                Ok(ReferenceJsonValue::Number(value.into()))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(ReferenceJsonValue::Number(value.into()))
            }

            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(ReferenceJsonValue::Number)
                    .ok_or_else(|| E::custom("non-finite reference number"))
            }

            fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
                Ok(ReferenceJsonValue::String)
            }

            fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
                Ok(ReferenceJsonValue::String)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(ReferenceJsonValue::Array(values))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(key) = map.next_key()? {
                    values.push((key, map.next_value()?));
                }
                Ok(ReferenceJsonValue::Object(values))
            }
        }

        deserializer.deserialize_any(ReferenceVisitor)
    }
}

fn collect_reference_numbers(
    value: &ReferenceJsonValue,
    path: &mut Vec<qubit_json::decode::JsonPathSegment>,
    output: &mut Vec<(Vec<qubit_json::decode::JsonPathSegment>, serde_json::Number)>,
) {
    use qubit_json::decode::JsonPathSegment;

    match value {
        ReferenceJsonValue::Number(number) => output.push((path.clone(), number.clone())),
        ReferenceJsonValue::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                path.push(JsonPathSegment::Index(index));
                collect_reference_numbers(value, path, output);
                path.pop();
            }
        }
        ReferenceJsonValue::Object(values) => {
            for (key, value) in values {
                path.push(JsonPathSegment::Key(key.clone()));
                collect_reference_numbers(value, path, output);
                path.pop();
            }
        }
        ReferenceJsonValue::Null | ReferenceJsonValue::Bool | ReferenceJsonValue::String => {}
    }
}

/// Applies an independent checked-i128 decimal oracle to the supported
/// reference domain; enormous coefficients or exponents are explicitly skipped.
fn assert_exact_integer_oracle(input: &[u8]) {
    let options = qubit_json::decode::JsonValidatedDecodeOptions::builder().build();
    let mut decoder = JsonDecoder::with_limits(decode_limits(input, usize::MAX));
    let mut in_reference_domain = true;
    let mut all_exact = true;
    let mut expected_values = Vec::new();
    let result = decoder.decode_unique_validated_utf8::<std::convert::Infallible, _>(input, &options, |path, token| {
        match reference_exact_integer(token.raw()) {
            Some(Some(value)) => expected_values.push((path.to_vec(), value)),
            Some(None) => all_exact = false,
            None => in_reference_domain = false,
        }
        Ok(JsonNumberPolicy::ExactSafeInteger)
    });
    if in_reference_domain {
        assert_eq!(
            result.is_ok(),
            all_exact,
            "exact-integer policy must match the independent oracle"
        );
        if let Ok(owner) = &result {
            for (path, expected) in expected_values {
                let actual = value_at_path(owner.as_value(), &path).and_then(serde_json::Value::as_i64);
                assert_eq!(actual, Some(expected), "exact-safe-integer result at {path:?}");
            }
        }
    }
    if result.is_err() {
        assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
        assert_eq!(
            decoder.session().input_budget().map(|budget| budget.used()),
            Some(input.len())
        );
    }
}

/// Looks up a value using a typed path without recursively traversing the tree.
fn value_at_path<'value>(
    mut value: &'value serde_json::Value,
    path: &[qubit_json::decode::JsonPathSegment],
) -> Option<&'value serde_json::Value> {
    for segment in path {
        value = match (segment, value) {
            (qubit_json::decode::JsonPathSegment::Key(key), serde_json::Value::Object(object)) => object.get(key)?,
            (qubit_json::decode::JsonPathSegment::Index(index), serde_json::Value::Array(array)) => {
                array.get(*index)?
            }
            _ => return None,
        };
    }
    Some(value)
}

/// Returns `None` outside the checked-i128 reference domain, otherwise the
/// exact safe integer or `Some(None)` when the token is not safe/integral.
fn reference_exact_integer(raw: &str) -> Option<Option<i64>> {
    const MAX_SAFE: i128 = 9_007_199_254_740_991;
    let (mantissa, exponent) = match raw.find(['e', 'E']) {
        Some(index) => (&raw[..index], raw[index + 1..].parse::<i32>().ok()?),
        None => (raw, 0),
    };
    let negative = mantissa.starts_with('-');
    let digits = mantissa.strip_prefix('-').unwrap_or(mantissa);
    let fractional = digits.find('.').map_or(0_i32, |index| {
        i32::try_from(digits.len() - index - 1).unwrap_or(i32::MAX)
    });
    let coefficient = digits.replace('.', "").parse::<i128>().ok()?;
    let scale = exponent.checked_sub(fractional)?;
    let magnitude = if scale >= 0 {
        coefficient.checked_mul(10_i128.checked_pow(scale as u32)?)?
    } else {
        let divisor = 10_i128.checked_pow(scale.unsigned_abs())?;
        if coefficient % divisor != 0 {
            return Some(None);
        }
        coefficient / divisor
    };
    if magnitude > MAX_SAFE {
        return Some(None);
    }
    let signed = if negative { -magnitude } else { magnitude };
    Some(Some(i64::try_from(signed).ok()?))
}

/// Checks token preservation, output-limit boundaries, and writer parity.
pub fn assert_token_format_invariants(input: &[u8]) {
    if input.len() > 4096 {
        return;
    }
    let options = JsonPrettyOptions::builder()
        .indent_width(2)
        .max_indent_levels(Some(3))
        .trailing_newline(false)
        .build()
        .expect("fixed formatting options are valid");
    const MAX_OUTPUT: usize = 131_072;
    let Ok(output) = format_json_tokens(input, &options, Some(MAX_OUTPUT)) else {
        let mut writer = Vec::new();
        assert!(write_json_tokens(&mut writer, input, &options, Some(MAX_OUTPUT)).is_err());
        assert!(writer.is_empty(), "planning failures must not write a prefix");
        return;
    };
    let mut writer = Vec::new();
    write_json_tokens(&mut writer, input, &options, Some(MAX_OUTPUT))
        .expect("the same valid plan must write successfully");
    assert_eq!(writer, output);
    let source_tokens = remove_external_json_whitespace(input);
    assert_eq!(remove_external_json_whitespace(&output), source_tokens);
    assert!(format_json_tokens(input, &options, Some(output.len())).is_ok());
    if !output.is_empty() {
        assert!(format_json_tokens(input, &options, Some(output.len() - 1)).is_err());
    }
}

/// Checks validated node and source-string limits independently, including
/// decoded object keys and quote-excluded encoded byte lengths.
pub fn assert_validated_limit_invariants() {
    let decode = |input: &[u8], options: &qubit_json::decode::JsonValidatedDecodeOptions| {
        let mut decoder = JsonDecoder::with_limits(decode_limits(input, usize::MAX));
        let result = decoder.decode_unique_validated_utf8::<std::convert::Infallible, _>(input, options, |_, _| {
            Ok(JsonNumberPolicy::Preserve)
        });
        (result, decoder)
    };

    let string = br#""ab""#;
    let accepted = qubit_json::decode::JsonValidatedDecodeOptions::builder()
        .max_value_and_key_nodes(1)
        .max_encoded_string_bytes(2)
        .build();
    assert!(
        decode(string, &accepted).0.is_ok(),
        "root string uses one node and two token bytes"
    );
    let no_nodes = qubit_json::decode::JsonValidatedDecodeOptions::builder()
        .max_value_and_key_nodes(0)
        .build();
    assert_limit_failure(
        &decode(string, &no_nodes),
        string,
        "node limit is enforced independently",
    );
    let short_string = qubit_json::decode::JsonValidatedDecodeOptions::builder()
        .max_encoded_string_bytes(1)
        .build();
    assert_limit_failure(
        &decode(string, &short_string),
        string,
        "string token limit excludes quotes",
    );

    let object = br#"{"k":"v"}"#;
    let object_limit = qubit_json::decode::JsonValidatedDecodeOptions::builder()
        .max_value_and_key_nodes(3)
        .max_encoded_string_bytes(1)
        .build();
    assert!(
        decode(object, &object_limit).0.is_ok(),
        "object, key, and value each consume one node"
    );
    let missing_key_node = qubit_json::decode::JsonValidatedDecodeOptions::builder()
        .max_value_and_key_nodes(2)
        .build();
    assert_limit_failure(
        &decode(object, &missing_key_node),
        object,
        "decoded object keys count as nodes",
    );

    let escaped = br#""\u0061""#;
    let encoded_length = qubit_json::decode::JsonValidatedDecodeOptions::builder()
        .max_encoded_string_bytes(6)
        .build();
    assert!(
        decode(escaped, &encoded_length).0.is_ok(),
        "string limit counts source token bytes"
    );
    let decoded_length = qubit_json::decode::JsonValidatedDecodeOptions::builder()
        .max_encoded_string_bytes(1)
        .build();
    assert_limit_failure(
        &decode(escaped, &decoded_length),
        escaped,
        "escaped source length exceeds decoded length",
    );
}

fn assert_limit_failure(result: &ValidatedLimitResult, input: &[u8], message: &str) {
    assert!(result.0.is_err(), "{message}");
    assert_eq!(result.1.session().value_budget().used_nodes(), Some(0), "{message}");
    assert_eq!(
        result.1.session().input_budget().map(|budget| budget.used()),
        Some(input.len()),
        "{message}",
    );
}

/// Removes only JSON whitespace outside strings, preserving all token bytes.
fn remove_external_json_whitespace(input: &[u8]) -> Vec<u8> {
    let mut inside = false;
    let mut escaped = false;
    let mut tokens = Vec::with_capacity(input.len());
    for &byte in input {
        if inside {
            tokens.push(byte);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                inside = false;
            }
        } else if !matches!(byte, b' ' | b'\n' | b'\r' | b'\t') {
            tokens.push(byte);
            inside = byte == b'"';
        }
    }
    tokens
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

fn error_signature<T>(result: &Result<T, JsonDecodeError>) -> Option<DecodeErrorSignature> {
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
            assert_eq!(
                actual.is_i64(),
                expected.is_i64(),
                "signed-integer category must be preserved"
            );
            assert_eq!(
                actual.is_u64(),
                expected.is_u64(),
                "unsigned-integer category must be preserved"
            );
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
