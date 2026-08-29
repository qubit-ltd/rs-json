// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public [`qubit_json::decode::NormalizingJsonDecoder`] API.

use qubit_budget::ResourceBudget;
use qubit_budget::ResourceLimit;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::decode::DiagnosticPolicy;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecodeStage;
use qubit_json::decode::JsonRootKind;
use qubit_json::decode::MarkdownFencePolicy;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;
use serde::de::DeserializeOwned;
use serde_json::Error as JsonError;
use serde_json::Value;
use serde_json::json;
use serde_json::value::RawValue;

use crate::fixtures::ByteBuffer;
use crate::fixtures::CountedFailure;
use crate::fixtures::ExactInteger;
use crate::fixtures::Message;
use crate::fixtures::SingleValue;
use crate::fixtures::User;
use crate::fixtures::deserialize_calls;
use crate::fixtures::reset_deserialize_calls;

/// Creates a policy that exercises the normalization facade without rewriting.
fn no_normalization_policy() -> NormalizingJsonDecodePolicy {
    NormalizingJsonDecodePolicy::builder()
        .trim_whitespace(false)
        .strip_utf8_bom(false)
        .markdown_fence_policy(MarkdownFencePolicy::Disabled)
        .escape_control_chars_in_strings(false)
        .build()
}

/// Creates an owned decode session with the supplied value limits.
///
/// # Parameters
///
/// * `limits` - Value-resource limits enforced by the returned session.
///
/// # Returns
///
/// A fresh session with no input-byte limits and the supplied value limits.
fn value_budget_session(limits: JsonValueLimits) -> JsonDecodeSession<'static, JsonResource> {
    JsonDecodeSession::from_limits(
        JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
            .value_limits(limits)
            .build(),
    )
}

/// Runs one decode with a caller-owned session and restores the session after
/// the stateful decoder completes.
fn run_with_session<'a, T>(
    decoder: &NormalizingJsonDecoder<'_>,
    input: &str,
    session: &mut JsonDecodeSession<'a, JsonResource>,
) -> Result<T, JsonDecodeError>
where
    T: DeserializeOwned,
{
    let owned_session = std::mem::replace(session, JsonDecodeSession::from_limits(JsonDecodeLimits::new()));
    let mut stateful = NormalizingJsonDecoder::new(decoder.policy().clone(), owned_session);
    let result = stateful.decode_str(input);
    *session = stateful.into_session();
    result
}

/// Verifies that owned exposes the configured policy.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_owned_exposes_configured_policy() {
    let policy = NormalizingJsonDecodePolicy::builder()
        .markdown_fence_policy(MarkdownFencePolicy::Disabled)
        .build();
    let decoder = NormalizingJsonDecoder::with_limits(
        policy.clone(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    assert_eq!(decoder.policy(), &policy);
}

/// Verifies that owned accepts the default policy.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_owned_accepts_default_policy() {
    let decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    assert_eq!(decoder.policy(), &NormalizingJsonDecodePolicy::default());
}

/// Verifies decoding ignores value limits unless configured in the limits.
#[test]
fn test_decode_without_value_limits_ignores_structure() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder().build(),
        JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
            .max_input_bytes(256)
            .max_normalized_input_bytes(256)
            .build(),
    );

    decoder
        .decode_str::<Value>("[[[[null]]]]")
        .expect("without value limits, nested structure is accepted within byte cap");
}

/// Verifies convenience decode enforces configured value limits.
#[test]
fn test_decode_with_value_limits_rejects_excessive_depth() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder().build(),
        JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
            .value_limits(JsonValueLimits::<JsonResource, usize>::builder().max_depth(1).build())
            .build(),
    );
    let error = decoder
        .decode_str::<Value>("[null]")
        .expect_err("depth limit must reject nested values on convenience decode");

    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.stage(), JsonDecodeStage::Admission);
}

/// Verifies `decode_value` applies configured value limits.
#[test]
fn test_decode_value_with_value_limits_rejects_excessive_nodes() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder().build(),
        JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
            .value_limits(JsonValueLimits::<JsonResource, usize>::builder().max_nodes(1).build())
            .build(),
    );
    let error = decoder
        .decode_value("[null]")
        .expect_err("node limit must reject nested array values");

    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.stage(), JsonDecodeStage::Admission);
}

/// Verifies that callers can share one budget session with lenient decoding.
#[test]
fn test_stateful_decoder_charges_caller_owned_input_budget() {
    let decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let mut input = ResourceBudget::from_limit(ResourceLimit::new(JsonResource::InputBytes, 16));
    let mut value = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::builder().build());
    let mut session = JsonDecodeSession::borrowing_input(&mut input, &mut value);

    let decoded: Value =
        run_with_session(&decoder, "{\"ok\":true}", &mut session).expect("caller-owned session must be accepted");

    assert_eq!(decoded, json!({"ok": true}));
    assert_eq!(input.used(), 11);
}

/// Verifies that value admission charges exact resources once and accumulates
/// charges when the same session is reused.
///
/// # Panics
///
/// Panics when either decode fails or any value counter differs from the
/// normalized JSON measurements.
#[test]
fn test_stateful_decoder_charges_exact_value_resources_cumulatively() {
    let decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_depth(3)
        .max_nodes(6)
        .max_sequence_items(2)
        .max_map_entries(1)
        .max_key_bytes(5)
        .max_string_bytes(2)
        .max_number_bytes(4)
        .max_payload_bytes(13)
        .build();
    let mut session = value_budget_session(limits);

    let first: Value = run_with_session(&decoder, r#"{"items":["é",1e+3]}"#, &mut session)
        .expect("the first value must fit every exact point limit");
    assert_eq!(first["items"][0], json!("é"));
    assert_eq!(first["items"][1].as_f64(), Some(1_000.0));
    assert_eq!(session.value_budget().used_nodes(), Some(4));
    assert_eq!(session.value_budget().used_payload_bytes(), Some(11),);

    let second: Value = run_with_session(&decoder, r#"{"k":"v"}"#, &mut session)
        .expect("the second value must consume the remaining session budget");
    assert_eq!(second, json!({"k": "v"}));
    assert_eq!(session.value_budget().used_nodes(), Some(6));
    assert_eq!(session.value_budget().used_payload_bytes(), Some(13),);
}

/// Exercises strict lexical preflight branches for literals, containers,
/// numbers, escapes, and malformed UTF-8 boundaries.
#[test]
fn test_strict_decode_exercises_lexical_error_shapes() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        no_normalization_policy(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    for input in ["false", "[]", "{}"] {
        let mut session = value_budget_session(JsonValueLimits::<JsonResource, usize>::builder().build());
        run_with_session::<Value>(&decoder, input, &mut session)
            .expect("strict scalar and empty containers are valid JSON");
    }
    for input in [
        "[",
        "[1,]",
        "[1 2]",
        "{",
        "{\"key\"}",
        "{\"key\":}",
        "{\"key\" 1}",
        "\"\\uD800\"",
        "\"\\u12\"",
        "-",
        "01",
        "1e",
    ] {
        let mut session = value_budget_session(JsonValueLimits::<JsonResource, usize>::builder().build());
        assert!(
            run_with_session::<Value>(&decoder, input, &mut session).is_err(),
            "strict input should be rejected: {input:?}",
        );
    }

    let error = decoder
        .decode_utf8::<Value>(&[b'"', 0xff, b'"'])
        .expect_err("invalid UTF-8 must be rejected before parsing");
    assert!(error.to_string().contains("UTF-8"));
}

/// Verifies that every limited value resource produces a structured admission
/// failure.
///
/// # Panics
///
/// Panics when a constrained input succeeds or reports the wrong public error
/// classification or resource identity.
#[test]
fn test_stateful_decoder_classifies_each_value_budget_rejection() {
    let decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let cases = [
        (
            JsonValueLimits::<JsonResource, usize>::builder()
                .max_map_entries(1)
                .build(),
            r#"{"a":null,"b":null}"#,
            JsonResource::MapEntries,
            "object entry budget must reject two entries",
        ),
        (
            JsonValueLimits::<JsonResource, usize>::builder()
                .max_sequence_items(1)
                .build(),
            "[null,null]",
            JsonResource::SequenceItems,
            "array item budget must reject two items",
        ),
        (
            JsonValueLimits::<JsonResource, usize>::builder()
                .max_key_bytes(1)
                .build(),
            r#"{"ab":null}"#,
            JsonResource::KeyBytes,
            "key budget must reject the decoded key",
        ),
        (
            JsonValueLimits::<JsonResource, usize>::builder()
                .max_string_bytes(1)
                .build(),
            r#""ab""#,
            JsonResource::StringBytes,
            "string budget must reject the decoded string",
        ),
        (
            JsonValueLimits::<JsonResource, usize>::builder()
                .max_number_bytes(3)
                .build(),
            "1e+3",
            JsonResource::NumberBytes,
            "number budget must reject the lexical representation",
        ),
        (
            JsonValueLimits::<JsonResource, usize>::builder().max_nodes(1).build(),
            "[null]",
            JsonResource::Nodes,
            "node budget must reject the child value",
        ),
        (
            JsonValueLimits::<JsonResource, usize>::builder().max_depth(1).build(),
            "[null]",
            JsonResource::Depth,
            "depth budget must reject the nested value",
        ),
        (
            JsonValueLimits::<JsonResource, usize>::builder()
                .max_payload_bytes(2)
                .build(),
            r#"{"a":"bc"}"#,
            JsonResource::PayloadBytes,
            "payload budget must reject cumulative key and string bytes",
        ),
    ];

    for (limits, input, expected_resource, expectation) in cases {
        let mut session = value_budget_session(limits);
        let error = run_with_session::<Value>(&decoder, input, &mut session).expect_err(expectation);
        assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
        assert_eq!(error.stage(), JsonDecodeStage::Admission);
        assert_eq!(
            *error
                .budget_error()
                .expect("budget rejection details must be retained")
                .resource(),
            expected_resource,
        );
    }
}

/// Verifies that a rejected admission preserves only earlier committed session
/// charges.
///
/// # Panics
///
/// Panics when successful charges are rolled back, rejected staged charges are
/// published, or the session cannot use its remaining capacity.
#[test]
fn test_stateful_decoder_budget_rejection_preserves_committed_charges() {
    let decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(5)
        .max_payload_bytes(4)
        .build();
    let mut session = value_budget_session(limits);

    run_with_session::<Value>(&decoder, r#"{"a":"b"}"#, &mut session)
        .expect("the first value must fit the cumulative budgets");
    let error = run_with_session::<Value>(&decoder, r#"{"cc":"ddd"}"#, &mut session)
        .expect_err("the second string must exceed the payload budget");

    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.stage(), JsonDecodeStage::Admission);
    assert_eq!(
        *error
            .budget_error()
            .expect("budget rejection details must be retained")
            .resource(),
        JsonResource::PayloadBytes,
    );
    assert_eq!(session.value_budget().used_nodes(), Some(2));
    assert_eq!(session.value_budget().used_payload_bytes(), Some(2),);

    let value: Value = run_with_session(&decoder, "null", &mut session)
        .expect("the rejected session must retain its uncommitted value capacity");
    assert_eq!(value, Value::Null);
    assert_eq!(session.value_budget().used_nodes(), Some(3));
}

/// Verifies that fenced lenient input is value-accounted from its normalized
/// JSON representation.
///
/// # Panics
///
/// Panics when normalization, decoding, or any exact normalized counter does
/// not match the expected value.
#[test]
fn test_stateful_decoder_accounts_normalized_fenced_value() {
    const NORMALIZED: &str = r#"{"escaped":"\u4e2d","number":1e+3}"#;
    const INPUT: &str = "```json\n{\"escaped\":\"\\u4e2d\",\"number\":1e+3}\n```";

    let decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let mut input_budget = ResourceBudget::new(JsonResource::InputBytes, INPUT.len());
    let mut normalized_budget = ResourceBudget::new(JsonResource::NormalizedInputBytes, NORMALIZED.len());
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_depth(2)
        .max_nodes(3)
        .max_map_entries(2)
        .max_key_bytes(7)
        .max_string_bytes(3)
        .max_number_bytes(4)
        .max_payload_bytes(20)
        .build();
    let mut value_budget = JsonValueBudget::new(limits);
    let mut session = JsonDecodeSession::borrowing_all(&mut input_budget, &mut normalized_budget, &mut value_budget);

    let value: Value =
        run_with_session(&decoder, INPUT, &mut session).expect("normalized fenced JSON must fit its exact budgets");

    assert_eq!(value["escaped"], json!("中"));
    assert_eq!(value["number"].as_f64(), Some(1_000.0));
    assert_eq!(session.input_budget().expect("raw budget").used(), INPUT.len());
    assert_eq!(
        session.normalized_input_budget().expect("normalized budget").used(),
        NORMALIZED.len(),
    );
    assert_eq!(session.value_budget().used_nodes(), Some(3));
    assert_eq!(session.value_budget().used_payload_bytes(), Some(20),);
}

/// Verifies that session admission preserves lexical and target-type error
/// classifications.
///
/// # Panics
///
/// Panics when syntax or target deserialization failures are reported as
/// budget admission failures.
#[test]
fn test_stateful_decoder_preserves_non_budget_error_classification() {
    let decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(8)
        .max_payload_bytes(16)
        .build();

    let mut syntax_session = value_budget_session(limits);
    let syntax_error = run_with_session::<Value>(&decoder, r#"{"value":]"#, &mut syntax_session)
        .expect_err("malformed normalized JSON must remain a lexical error");
    assert_eq!(syntax_error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(syntax_error.stage(), JsonDecodeStage::Parse);
    assert!(syntax_error.budget_error().is_none());

    let mut target_session = value_budget_session(limits);
    let target_error = run_with_session::<Message>(&decoder, r#"{"text":7}"#, &mut target_session)
        .expect_err("valid JSON with the wrong target type must remain a deserialize error");
    assert_eq!(target_error.kind(), JsonDecodeErrorKind::Deserialize);
    assert_eq!(target_error.stage(), JsonDecodeStage::Deserialize);
    assert!(target_error.budget_error().is_none());
    assert_eq!(target_session.value_budget().used_nodes(), Some(0),);
}

/// Verifies malformed normalized JSON retains raw and normalized input while
/// discarding staged value admission before a shared session is reused.
#[test]
fn test_stateful_decoder_syntax_failure_retains_input_and_reuses_value_budget() {
    let rejected = r#"{"value":]}"#;
    let accepted = "null";
    let mut session = JsonDecodeSession::from_limits(
        JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
            .max_input_bytes(rejected.len() + accepted.len())
            .max_normalized_input_bytes(rejected.len() + accepted.len())
            .max_nodes(1)
            .build(),
    );
    let decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );

    let error = run_with_session::<Value>(&decoder, rejected, &mut session)
        .expect_err("malformed normalized JSON must be rejected");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(
        session.input_budget().expect("configured raw budget").used(),
        rejected.len()
    );
    assert_eq!(
        session
            .normalized_input_budget()
            .expect("configured normalized budget")
            .used(),
        rejected.len()
    );
    assert_eq!(session.value_budget().used_nodes(), Some(0));

    assert_eq!(
        run_with_session::<Value>(&decoder, accepted, &mut session)
            .expect("a later value must fit after syntax rollback"),
        Value::Null
    );
    assert_eq!(
        session.input_budget().expect("configured raw budget").used(),
        rejected.len() + accepted.len()
    );
    assert_eq!(
        session
            .normalized_input_budget()
            .expect("configured normalized budget")
            .used(),
        rejected.len() + accepted.len()
    );
    assert_eq!(session.value_budget().used_nodes(), Some(1));
}

/// Verifies lenient admission retains input but discards all staged value
/// accounting when a value limit rejects the attempt.
#[test]
fn test_stateful_decoder_budget_rejection_rolls_back_value() {
    let input = "[null]";
    let mut session = JsonDecodeSession::from_limits(
        JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
            .max_input_bytes(input.len())
            .max_normalized_input_bytes(input.len())
            .max_nodes(1)
            .build(),
    );

    let decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = run_with_session::<Value>(&decoder, input, &mut session)
        .expect_err("two nodes must exceed the one-node value budget");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(
        session.input_budget().expect("configured raw budget").used(),
        input.len()
    );
    assert_eq!(
        session
            .normalized_input_budget()
            .expect("configured normalized budget")
            .used(),
        input.len()
    );
    assert_eq!(session.value_budget().used_nodes(), Some(0));
}

/// Verifies that session admission preserves serde's syntax position for
/// ordinary malformed JSON.
///
/// # Panics
///
/// Panics when lexical admission replaces serde's stable syntax position.
#[test]
fn test_stateful_decoder_preserves_serde_syntax_position() {
    let mut session = value_budget_session(JsonValueLimits::<JsonResource, usize>::builder().max_nodes(2).build());
    let decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder()
            .diagnostic_policy(DiagnosticPolicy::Detailed)
            .build(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = run_with_session::<Value>(&decoder, "{", &mut session)
        .expect_err("an incomplete object must return an invalid JSON error");

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(error.stage(), JsonDecodeStage::Parse);
    assert_eq!(error.line(), Some(1));
    assert_eq!(error.column(), Some(2));
    let source = std::error::Error::source(&error).expect("detailed ordinary syntax errors must retain their source");
    assert!(source.downcast_ref::<JsonError>().is_some());
}

/// Verifies that lexical surrogate rejection returns stable errors for target
/// types accepted by different serde JSON paths.
///
/// # Panics
///
/// Panics when an unpaired surrogate unwinds, reports unstable diagnostics, or
/// changes the preflight accounting and privacy semantics.
#[test]
fn test_stateful_decoder_rejects_unpaired_surrogate_without_panicking() {
    const INPUT: &str = r#""\ud800""#;
    let limits = JsonValueLimits::<JsonResource, usize>::builder().max_nodes(2).build();

    let mut string_session = value_budget_session(limits);
    let string_decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let string_error = run_with_session::<String>(&string_decoder, INPUT, &mut string_session)
        .expect_err("an unpaired surrogate must return an invalid JSON error");
    assert_eq!(string_error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(string_error.stage(), JsonDecodeStage::Parse);
    assert_eq!(string_error.raw_input_bytes(), INPUT.len());
    assert_eq!(string_error.normalized_input_bytes(), Some(INPUT.len()));
    assert_eq!(string_error.line(), Some(1));
    assert_eq!(string_error.column(), Some(8));
    assert!(string_error.budget_error().is_none());
    assert!(std::error::Error::source(&string_error).is_none());
    assert_eq!(string_session.value_budget().used_nodes(), Some(0),);

    let decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder()
            .diagnostic_policy(DiagnosticPolicy::Detailed)
            .build(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let mut raw_value_session = value_budget_session(limits);
    let raw_value_error = run_with_session::<Box<RawValue>>(&decoder, INPUT, &mut raw_value_session)
        .expect_err("RawValue must not bypass lexical surrogate rejection");
    assert_eq!(raw_value_error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(raw_value_error.stage(), JsonDecodeStage::Parse);
    assert_eq!(raw_value_error.diagnostic_policy(), DiagnosticPolicy::Detailed);
    assert!(raw_value_error.budget_error().is_none());
    let source =
        std::error::Error::source(&raw_value_error).expect("detailed lexical errors must retain their stable source");
    assert!(source.to_string().contains("unpaired Unicode surrogate"));
    assert_eq!(raw_value_session.value_budget().used_nodes(), Some(0),);
}

/// Verifies that strict decoder preserves serde json grammar.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_strict_decoder_preserves_serde_json_grammar() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        no_normalization_policy(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );

    let canonical: Value = decoder
        .decode_str(" \n{\"ok\":true}\t")
        .expect("strict mode must preserve whitespace accepted by serde_json");
    assert_eq!(canonical, json!({"ok": true}));

    for input in [
        "\u{feff}{\"ok\":true}",
        "```json\n{\"ok\":true}\n```",
        "{\"text\":\"line one\nline two\"}",
    ] {
        let error = decoder
            .decode_value(input)
            .expect_err("strict mode must reject lenient-only input forms");
        assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
        assert_eq!(error.diagnostic_policy(), DiagnosticPolicy::Redacted,);
    }
}

/// Verifies that decode value parses normalized json.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_parses_normalized_json() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let value = decoder
        .decode_value("```json\n{\"name\":\"alice\",\"age\":30}\n```")
        .expect("default decoder should parse JSON wrapped in a Markdown code fence");
    assert_eq!(value, json!({"name": "alice", "age": 30}));
}

/// Verifies that decode typed value succeeds.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_typed_value_succeeds() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let person: User = decoder
        .decode_str("{\"name\":\"alice\",\"age\":30}")
        .expect("valid JSON object should deserialize into User");
    assert_eq!(
        person,
        User {
            name: "alice".to_string(),
            age: 30,
        }
    );
}

/// Verifies that decode slice decodes valid utf8 without changing semantics.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_utf8_decodes_valid_bytes_without_changing_semantics() {
    let value: Value = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    )
    .decode_utf8(b"{\"ok\":true}")
    .expect("valid UTF-8 JSON bytes must decode");
    assert_eq!(value, json!({"ok": true}));
}

/// Verifies that decode slice rejects invalid utf8 for byte target.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_utf8_rejects_invalid_utf8_for_byte_target() {
    let error = NormalizingJsonDecoder::with_limits(
        no_normalization_policy(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    )
    .decode_utf8::<ByteBuffer>(b"\"\xff\"")
    .expect_err("invalid UTF-8 must be rejected before byte deserialization");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidUtf8);
}

/// Verifies that decode slice invokes target deserializer once on failure.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_utf8_invokes_target_deserializer_once_on_failure() {
    reset_deserialize_calls();
    let error = NormalizingJsonDecoder::with_limits(
        no_normalization_policy(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    )
    .decode_utf8::<CountedFailure>(br#""value""#)
    .expect_err("the counted target intentionally rejects valid JSON");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    assert_eq!(deserialize_calls(), 1);
}

/// Verifies that decode slice accepts non rewrite strict overrides.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_utf8_accepts_non_rewrite_strict_overrides() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder()
            .diagnostic_policy(DiagnosticPolicy::Detailed)
            .build(),
        JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
            .max_input_bytes(64)
            .build(),
    );
    let value: Value = decoder
        .decode_utf8(b"{\"ok\":true}")
        .expect("non-rewrite policy must preserve successful byte decoding");
    assert_eq!(value, json!({"ok": true}));
}

/// Verifies that decode slice preserves deserialize error mapping.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_utf8_preserves_deserialize_error_mapping() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder()
            .diagnostic_policy(DiagnosticPolicy::Detailed)
            .build(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_utf8::<Message>(b"{\"text\":7}")
        .expect_err("valid JSON with the wrong field type must fail deserialization");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    assert_eq!(error.diagnostic_policy(), DiagnosticPolicy::Detailed);
    assert!(std::error::Error::source(&error).is_some());
}

/// Verifies that decode slice preserves invalid json mapping.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_utf8_preserves_invalid_json_mapping() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        no_normalization_policy(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_utf8::<Message>(b"{\"text\":\"broken\"")
        .expect_err("malformed typed JSON must remain an invalid JSON error");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode slice checks raw size before utf8.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_utf8_checks_raw_size_before_utf8() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder().build(),
        JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
            .max_input_bytes(1)
            .build(),
    );
    let error = decoder
        .decode_utf8::<Value>(&[0xff, 0xfe])
        .expect_err("raw size must be checked before UTF-8");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
}

/// Verifies that decode slice accepts input at exact raw size limit.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_utf8_accepts_input_at_exact_raw_size_limit() {
    let input = b"null";
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder().build(),
        JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
            .max_input_bytes(input.len())
            .build(),
    );

    let value = decoder
        .decode_utf8::<Value>(input)
        .expect("input at the exact raw byte limit must be accepted");

    assert_eq!(value, Value::Null);
}

/// Verifies that decode slice classifies invalid utf8.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_utf8_classifies_invalid_utf8() {
    let error = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    )
    .decode_utf8::<Value>(&[0xff])
    .expect_err("invalid UTF-8 must fail before normalization");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidUtf8);
    assert_eq!(error.stage(), JsonDecodeStage::DecodeText);
    assert_eq!(error.raw_input_bytes(), 1);
    assert_eq!(error.normalized_input_bytes(), None);
}

/// Verifies that decode reports empty input from normalizer.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_reports_empty_input_from_normalizer() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_str::<User>("")
        .expect_err("empty input should fail during normalization");
    assert_eq!(error.kind(), JsonDecodeErrorKind::EmptyInput);
}

/// Verifies that decode typed value applies normalization pipeline.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_typed_value_applies_normalization_pipeline() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let message: Message = decoder
        .decode_str("```json\n{\"text\":\"a\nb\"}\n```")
        .expect("typed decode should still normalize fenced JSON and repair string control chars");
    assert_eq!(
        message,
        Message {
            text: "a\nb".to_string(),
        }
    );
}

/// Verifies that decode object requires object top level.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_requires_object_top_level() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_object_str::<User>("[{\"name\":\"alice\",\"age\":30}]")
        .expect_err("top-level array should be rejected by decode_object");
    assert_eq!(error.kind(), JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.stage(), JsonDecodeStage::TopLevelCheck);
    assert_eq!(error.expected_top_level(), Some(JsonRootKind::Object));
    assert_eq!(error.actual_top_level(), Some(JsonRootKind::Array));
}

/// Verifies that decode object reports empty input from normalizer.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_reports_empty_input_from_normalizer() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_object_str::<User>("")
        .expect_err("empty input should fail during normalization");
    assert_eq!(error.kind(), JsonDecodeErrorKind::EmptyInput);
}

/// Verifies that decode object reports invalid json for malformed array.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_reports_invalid_json_for_malformed_array() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_object_str::<User>("[")
        .expect_err("malformed JSON should be reported before top-level checking");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode object reports invalid json for malformed scalar.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_reports_invalid_json_for_malformed_scalar() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_object_str::<User>("\"unterminated")
        .expect_err("malformed scalar JSON should not be treated as a top-level mismatch");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode array requires array top level.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_requires_array_top_level() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_array_str::<User>("{\"name\":\"alice\",\"age\":30}")
        .expect_err("top-level object should be rejected by decode_array");
    assert_eq!(error.kind(), JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level(), Some(JsonRootKind::Array));
    assert_eq!(error.actual_top_level(), Some(JsonRootKind::Object));
}

/// Verifies that decode array reports empty input from normalizer.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_reports_empty_input_from_normalizer() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_array_str::<User>("")
        .expect_err("empty input should fail during normalization");
    assert_eq!(error.kind(), JsonDecodeErrorKind::EmptyInput);
}

/// Verifies that decode array reports invalid json for malformed object.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_reports_invalid_json_for_malformed_object() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_array_str::<User>("{")
        .expect_err("malformed JSON should be reported before top-level checking");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode object rejects scalar top level.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_rejects_scalar_top_level() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_object_str::<User>("42")
        .expect_err("top-level scalar should be rejected by decode_object");
    assert_eq!(error.kind(), JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level(), Some(JsonRootKind::Object));
    assert_eq!(error.actual_top_level(), Some(JsonRootKind::Other));
}

/// Verifies that decode array rejects scalar top level.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_rejects_scalar_top_level() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_array_str::<User>("42")
        .expect_err("top-level scalar should be rejected by decode_array");
    assert_eq!(error.kind(), JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level(), Some(JsonRootKind::Array));
    assert_eq!(error.actual_top_level(), Some(JsonRootKind::Other));
}

/// Verifies that decode array succeeds.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_succeeds() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let people = decoder
        .decode_array_str::<User>("[{\"name\":\"alice\",\"age\":30}]")
        .expect("top-level array should deserialize into Vec<User>");
    assert_eq!(
        people,
        vec![User {
            name: "alice".to_string(),
            age: 30,
        }]
    );
}

/// Verifies that decode object reports deserialize error after top level check.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_reports_deserialize_error_after_top_level_check() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_object_str::<User>("{\"name\":\"alice\",\"age\":\"old\"}")
        .expect_err("valid object with wrong field type should return Deserialize");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
}

/// Verifies that decode array reports deserialize error after top level check.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_reports_deserialize_error_after_top_level_check() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_array_str::<User>("[{\"name\":\"alice\",\"age\":\"old\"}]")
        .expect_err("valid array with wrong element type should return Deserialize");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
}

/// Verifies that decode allows generic scalar targets.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_allows_generic_scalar_targets() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let value: i64 = decoder
        .decode_str("42")
        .expect("scalar JSON should deserialize into i64");
    assert_eq!(value, 42);
}

/// Verifies that decode reports invalid json.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_reports_invalid_json() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_str::<User>("{")
        .expect_err("broken JSON should return InvalidJson");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode reports deserialize error.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_reports_deserialize_error() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_str::<User>("{\"name\":\"alice\",\"age\":\"old\"}")
        .expect_err("JSON with a wrong field type should return Deserialize");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
}

/// Verifies that decode reports invalid json when data error precedes syntax
/// error.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_reports_invalid_json_when_data_error_precedes_syntax_error() {
    let error = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    )
    .decode_str::<SingleValue>("{\"value\":\"wrong\",")
    .expect_err("incomplete JSON must take precedence over a field type error");

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(error.stage(), JsonDecodeStage::Parse);
}

/// Verifies that decode object reports invalid json when data error precedes
/// syntax error.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_reports_invalid_json_when_data_error_precedes_syntax_error() {
    let error = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    )
    .decode_object_str::<SingleValue>("{\"value\":\"wrong\",")
    .expect_err("incomplete object JSON must take precedence over a field type error");

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(error.stage(), JsonDecodeStage::Parse);
}

/// Verifies that decode array reports invalid json when data error precedes
/// syntax error.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_reports_invalid_json_when_data_error_precedes_syntax_error() {
    let error = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    )
    .decode_array_str::<u8>("[\"wrong\",")
    .expect_err("incomplete array JSON must take precedence over an element type error");

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(error.stage(), JsonDecodeStage::Parse);
}

/// Verifies that decode object reports invalid json for non token start.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_reports_invalid_json_for_non_token_start() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder()
            .trim_whitespace(false)
            .markdown_fence_policy(MarkdownFencePolicy::Disabled)
            .build(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_object_str::<User>(" \n\t ")
        .expect_err("invalid syntax should still be mapped as InvalidJson");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decoder reuses configuration between calls.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decoder_reuses_configuration_between_calls() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder()
            .markdown_fence_policy(MarkdownFencePolicy::Disabled)
            .build(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );

    let first = decoder
        .decode_value("```json\n{\"a\":1}\n```")
        .expect_err("disabled fence stripping must reject the first input");
    assert_eq!(first.kind(), JsonDecodeErrorKind::InvalidJson);

    let second = decoder
        .decode_value("```json\n{\"a\":2}\n```")
        .expect_err("disabled fence stripping must reject the second input");
    assert_eq!(second.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decoders with different configs do not share state.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decoders_with_different_configs_do_not_share_state() {
    let mut strict_decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder()
            .markdown_fence_policy(MarkdownFencePolicy::Disabled)
            .build(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let mut permissive_decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );

    assert_eq!(
        strict_decoder
            .decode_value("```json\n{\"a\":1}\n```")
            .expect_err("code fence should stay when stripping is disabled")
            .kind(),
        JsonDecodeErrorKind::InvalidJson
    );
    let value = permissive_decoder
        .decode_value("```json\n{\"a\":1}\n```")
        .expect("default normalizer should strip one markdown fence");
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decoder keeps trim whitespace setting for empty text.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decoder_keeps_trim_whitespace_setting_for_empty_text() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder().trim_whitespace(false).build(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_value(" \n\t")
        .expect_err("trim disabled should leave whitespace for parser");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode object rejects integers above the public range.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_rejects_u128_outside_64_bit_range() {
    let error = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    )
    .decode_object_str::<ExactInteger>(r#"{"value":340282366920938463463374607431768211455}"#)
    .expect_err("direct object decoding must enforce the public integer range");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode array rejects integers above the public range.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_rejects_u128_outside_64_bit_range() {
    let error = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    )
    .decode_array_str::<ExactInteger>(r#"[{"value":340282366920938463463374607431768211455}]"#)
    .expect_err("direct array decoding must enforce the public integer range");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode object preserves duplicate field rejection.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_preserves_duplicate_field_rejection() {
    let error = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    )
    .decode_object_str::<SingleValue>(r#"{"value":1,"value":2}"#)
    .expect_err("direct object decoding should reject duplicate fields");

    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
}

/// Verifies Serde's depth guard is classified as target materialization after
/// normalized JSON passes lexical admission.
///
/// # Panics
///
/// Panics when the decoder reports a target-materialization limit as invalid
/// JSON syntax.
#[test]
fn test_decoder_classifies_serde_depth_as_deserialization() {
    let input = format!("{}0{}", "[".repeat(128), "]".repeat(128));
    let object = format!(r#"{{"value":{input}}}"#);
    let mut decoder = NormalizingJsonDecoder::with_limits(
        no_normalization_policy(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let errors = [
        decoder
            .decode_str::<Value>(&input)
            .expect_err("decode_str must expose target materialization depth"),
        decoder
            .decode_utf8::<Value>(input.as_bytes())
            .expect_err("decode_utf8 must expose target materialization depth"),
        decoder
            .decode_value(&input)
            .expect_err("decode_value must expose target materialization depth"),
        decoder
            .decode_array_str::<Value>(&input)
            .expect_err("decode_array must expose target materialization depth"),
        decoder
            .decode_object_str::<Value>(&object)
            .expect_err("decode_object must expose target materialization depth"),
    ];

    for error in errors {
        assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
        assert_eq!(error.stage(), JsonDecodeStage::Deserialize);
    }
}
