// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public API in `lenient_json_decoder.rs`.

use qubit_budget::ResourceBudget;
use qubit_budget::ResourceLimit;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::lenient::ErrorPrivacyPolicy;
use qubit_json::lenient::JsonDecodeErrorKind;
use qubit_json::lenient::JsonDecodeOptions;
use qubit_json::lenient::JsonDecodeStage;
use qubit_json::lenient::JsonTopLevelKind;
use qubit_json::lenient::LenientJsonDecoder;
use qubit_json::lenient::MarkdownFencePolicy;
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

/// Creates an owned decode session with the supplied value limits.
///
/// # Parameters
///
/// * `limits` - Value-resource limits enforced by the returned session.
///
/// # Returns
///
/// A fresh session with no input-byte limits and the supplied value limits.
fn value_budget_session(
    limits: JsonValueLimits,
) -> JsonDecodeSession<'static, JsonResource> {
    JsonDecodeSession::owned(
        JsonDecodeLimits::empty().with_value_limits(limits),
    )
}

/// Verifies that new exposes configured options.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_new_exposes_configured_options() {
    let options = JsonDecodeOptions::default()
        .with_markdown_fence_policy(MarkdownFencePolicy::Disabled);
    let decoder = LenientJsonDecoder::new(options.clone());
    assert_eq!(decoder.options(), &options);
}

/// Verifies that default uses default options.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_default_uses_default_options() {
    let decoder = LenientJsonDecoder::default();
    assert_eq!(decoder.options(), &JsonDecodeOptions::default());
}

/// Verifies that callers can share one budget session with lenient decoding.
#[test]
fn test_decode_with_session_charges_caller_owned_input_budget() {
    let decoder = LenientJsonDecoder::default();
    let mut input = ResourceBudget::from_limit(ResourceLimit::new(
        JsonResource::InputBytes,
        16,
    ));
    let mut value = JsonValueBudget::new(JsonValueLimits::empty());
    let mut session =
        JsonDecodeSession::borrowing_input(&mut input, &mut value);

    let decoded: Value = decoder
        .decode_with_session("{\"ok\":true}", &mut session)
        .expect("caller-owned session must be accepted");

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
fn test_decode_with_session_charges_exact_value_resources_cumulatively() {
    let decoder = LenientJsonDecoder::default();
    let limits = JsonValueLimits::empty()
        .with_max_depth(3)
        .with_max_nodes(6)
        .with_max_sequence_items(2)
        .with_max_map_entries(1)
        .with_max_key_bytes(5)
        .with_max_string_bytes(2)
        .with_max_number_bytes(4)
        .with_max_payload_bytes(13);
    let mut session = value_budget_session(limits);

    let first: Value = decoder
        .decode_with_session(r#"{"items":["é",1e+3]}"#, &mut session)
        .expect("the first value must fit every exact point limit");
    assert_eq!(first["items"][0], json!("é"));
    assert_eq!(first["items"][1].as_f64(), Some(1_000.0));
    assert_eq!(session.value_budget().structure_budget().used_nodes(), 4);
    assert_eq!(
        session
            .value_budget()
            .payload_budget()
            .expect("payload budget must be configured")
            .used(),
        11,
    );

    let second: Value = decoder
        .decode_with_session(r#"{"k":"v"}"#, &mut session)
        .expect("the second value must consume the remaining session budget");
    assert_eq!(second, json!({"k": "v"}));
    assert_eq!(session.value_budget().structure_budget().used_nodes(), 6);
    assert_eq!(
        session
            .value_budget()
            .payload_budget()
            .expect("payload budget must be configured")
            .used(),
        13,
    );
}

/// Verifies that every limited value resource produces a structured admission
/// failure.
///
/// # Panics
///
/// Panics when a constrained input succeeds or reports the wrong public error
/// classification or resource identity.
#[test]
fn test_decode_with_session_classifies_each_value_budget_rejection() {
    let decoder = LenientJsonDecoder::default();
    let cases = [
        (
            JsonValueLimits::empty().with_max_map_entries(1),
            r#"{"a":null,"b":null}"#,
            JsonResource::MapEntries,
            "object entry budget must reject two entries",
        ),
        (
            JsonValueLimits::empty().with_max_sequence_items(1),
            "[null,null]",
            JsonResource::SequenceItems,
            "array item budget must reject two items",
        ),
        (
            JsonValueLimits::empty().with_max_key_bytes(1),
            r#"{"ab":null}"#,
            JsonResource::KeyBytes,
            "key budget must reject the decoded key",
        ),
        (
            JsonValueLimits::empty().with_max_string_bytes(1),
            r#""ab""#,
            JsonResource::StringBytes,
            "string budget must reject the decoded string",
        ),
        (
            JsonValueLimits::empty().with_max_number_bytes(3),
            "1e+3",
            JsonResource::NumberBytes,
            "number budget must reject the lexical representation",
        ),
        (
            JsonValueLimits::empty().with_max_nodes(1),
            "[null]",
            JsonResource::Nodes,
            "node budget must reject the child value",
        ),
        (
            JsonValueLimits::empty().with_max_depth(1),
            "[null]",
            JsonResource::Depth,
            "depth budget must reject the nested value",
        ),
        (
            JsonValueLimits::empty().with_max_payload_bytes(2),
            r#"{"a":"bc"}"#,
            JsonResource::PayloadBytes,
            "payload budget must reject cumulative key and string bytes",
        ),
    ];

    for (limits, input, expected_resource, expectation) in cases {
        let mut session = value_budget_session(limits);
        let error = decoder
            .decode_with_session::<Value>(input, &mut session)
            .expect_err(expectation);
        assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
        assert_eq!(error.stage(), JsonDecodeStage::Admission);
        assert_eq!(
            *error
                .measured_budget_error()
                .expect("budget rejection details must be retained")
                .resource(),
            expected_resource,
        );
    }
}

/// Verifies that a rejected admission preserves every earlier session charge.
///
/// # Panics
///
/// Panics when successful or partial admission charges are rolled back, or
/// when the rejected session cannot use its remaining capacity.
#[test]
fn test_decode_with_session_budget_rejection_preserves_partial_charges() {
    let decoder = LenientJsonDecoder::default();
    let limits = JsonValueLimits::empty()
        .with_max_nodes(5)
        .with_max_payload_bytes(4);
    let mut session = value_budget_session(limits);

    decoder
        .decode_with_session::<Value>(r#"{"a":"b"}"#, &mut session)
        .expect("the first value must fit the cumulative budgets");
    let error = decoder
        .decode_with_session::<Value>(r#"{"cc":"ddd"}"#, &mut session)
        .expect_err("the second string must exceed the payload budget");

    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.stage(), JsonDecodeStage::Admission);
    assert_eq!(
        *error
            .measured_budget_error()
            .expect("budget rejection details must be retained")
            .resource(),
        JsonResource::PayloadBytes,
    );
    assert_eq!(session.value_budget().structure_budget().used_nodes(), 4);
    assert_eq!(
        session
            .value_budget()
            .payload_budget()
            .expect("payload budget must be configured")
            .used(),
        4,
    );

    let value: Value = decoder
        .decode_with_session("null", &mut session)
        .expect("the rejected session must retain one remaining node");
    assert_eq!(value, Value::Null);
    assert_eq!(session.value_budget().structure_budget().used_nodes(), 5);
}

/// Verifies that fenced lenient input is value-accounted from its normalized
/// JSON representation.
///
/// # Panics
///
/// Panics when normalization, decoding, or any exact normalized counter does
/// not match the expected value.
#[test]
fn test_decode_with_session_accounts_normalized_fenced_value() {
    const NORMALIZED: &str = r#"{"escaped":"\u4e2d","number":1e+3}"#;
    const INPUT: &str =
        "```json\n{\"escaped\":\"\\u4e2d\",\"number\":1e+3}\n```";

    let decoder = LenientJsonDecoder::default();
    let mut input_budget =
        ResourceBudget::new(JsonResource::InputBytes, INPUT.len());
    let mut normalized_budget = ResourceBudget::new(
        JsonResource::NormalizedInputBytes,
        NORMALIZED.len(),
    );
    let limits = JsonValueLimits::empty()
        .with_max_depth(2)
        .with_max_nodes(3)
        .with_max_map_entries(2)
        .with_max_key_bytes(7)
        .with_max_string_bytes(3)
        .with_max_number_bytes(4)
        .with_max_payload_bytes(20);
    let mut value_budget = JsonValueBudget::new(limits);
    let mut session = JsonDecodeSession::borrowing_all(
        &mut input_budget,
        &mut normalized_budget,
        &mut value_budget,
    );

    let value: Value = decoder
        .decode_with_session(INPUT, &mut session)
        .expect("normalized fenced JSON must fit its exact budgets");

    assert_eq!(value["escaped"], json!("中"));
    assert_eq!(value["number"].as_f64(), Some(1_000.0));
    assert_eq!(
        session.input_budget().expect("raw budget").used(),
        INPUT.len()
    );
    assert_eq!(
        session
            .normalized_input_budget()
            .expect("normalized budget")
            .used(),
        NORMALIZED.len(),
    );
    assert_eq!(session.value_budget().structure_budget().used_nodes(), 3);
    assert_eq!(
        session
            .value_budget()
            .payload_budget()
            .expect("payload budget")
            .used(),
        20,
    );
}

/// Verifies that session admission preserves lexical and target-type error
/// classifications.
///
/// # Panics
///
/// Panics when syntax or target deserialization failures are reported as
/// budget admission failures.
#[test]
fn test_decode_with_session_preserves_non_budget_error_classification() {
    let decoder = LenientJsonDecoder::default();
    let limits = JsonValueLimits::empty()
        .with_max_nodes(8)
        .with_max_payload_bytes(16);

    let mut syntax_session = value_budget_session(limits);
    let syntax_error = decoder
        .decode_with_session::<Value>(r#"{"value":]"#, &mut syntax_session)
        .expect_err("malformed normalized JSON must remain a lexical error");
    assert_eq!(syntax_error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(syntax_error.stage(), JsonDecodeStage::Parse);
    assert!(syntax_error.measured_budget_error().is_none());

    let mut target_session = value_budget_session(limits);
    let target_error = decoder
        .decode_with_session::<Message>(r#"{"text":7}"#, &mut target_session)
        .expect_err("valid JSON with the wrong target type must remain a deserialize error");
    assert_eq!(target_error.kind(), JsonDecodeErrorKind::Deserialize);
    assert_eq!(target_error.stage(), JsonDecodeStage::Deserialize);
    assert!(target_error.measured_budget_error().is_none());
    assert_eq!(
        target_session
            .value_budget()
            .structure_budget()
            .used_nodes(),
        2,
    );
}

/// Verifies that session admission preserves serde's syntax position for
/// ordinary malformed JSON.
///
/// # Panics
///
/// Panics when lexical admission replaces serde's stable syntax position.
#[test]
fn test_decode_with_session_preserves_serde_syntax_position() {
    let mut session =
        value_budget_session(JsonValueLimits::empty().with_max_nodes(2));
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    );
    let error = decoder
        .decode_with_session::<Value>("{", &mut session)
        .expect_err("an incomplete object must return an invalid JSON error");

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(error.stage(), JsonDecodeStage::Parse);
    assert_eq!(error.normalized_line(), Some(1));
    assert_eq!(error.normalized_column(), Some(1));
    let source = std::error::Error::source(&error)
        .expect("detailed ordinary syntax errors must retain their source");
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
fn test_decode_with_session_rejects_unpaired_surrogate_without_panicking() {
    const INPUT: &str = r#""\ud800""#;
    let limits = JsonValueLimits::empty().with_max_nodes(2);

    let mut string_session = value_budget_session(limits);
    let string_error = LenientJsonDecoder::default()
        .decode_with_session::<String>(INPUT, &mut string_session)
        .expect_err("an unpaired surrogate must return an invalid JSON error");
    assert_eq!(string_error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(string_error.stage(), JsonDecodeStage::Parse);
    assert_eq!(string_error.raw_input_bytes(), INPUT.len());
    assert_eq!(string_error.normalized_input_bytes(), Some(INPUT.len()));
    assert_eq!(string_error.normalized_line(), Some(1));
    assert_eq!(string_error.normalized_column(), Some(8));
    assert!(string_error.measured_budget_error().is_none());
    assert!(std::error::Error::source(&string_error).is_none());
    assert_eq!(
        string_session
            .value_budget()
            .structure_budget()
            .used_nodes(),
        1,
    );

    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    );
    let mut raw_value_session = value_budget_session(limits);
    let raw_value_error = decoder
        .decode_with_session::<Box<RawValue>>(INPUT, &mut raw_value_session)
        .expect_err("RawValue must not bypass lexical surrogate rejection");
    assert_eq!(raw_value_error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(raw_value_error.stage(), JsonDecodeStage::Parse);
    assert_eq!(raw_value_error.privacy_policy(), ErrorPrivacyPolicy::Detailed);
    assert!(raw_value_error.measured_budget_error().is_none());
    let source = std::error::Error::source(&raw_value_error)
        .expect("detailed lexical errors must retain their stable source");
    assert!(source.to_string().contains("unpaired Unicode surrogate"));
    assert_eq!(
        raw_value_session
            .value_budget()
            .structure_budget()
            .used_nodes(),
        1,
    );
}

/// Verifies that strict decoder preserves serde json grammar.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_strict_decoder_preserves_serde_json_grammar() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions::strict());

    let canonical: Value = decoder
        .decode(" \n{\"ok\":true}\t")
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
        assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Redacted,);
    }
}

/// Verifies that decode value parses normalized json.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_parses_normalized_json() {
    let decoder = LenientJsonDecoder::default();
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
    let decoder = LenientJsonDecoder::default();
    let person: User = decoder
        .decode("{\"name\":\"alice\",\"age\":30}")
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
fn test_decode_slice_decodes_valid_utf8_without_changing_semantics() {
    let value: Value = LenientJsonDecoder::default()
        .decode_slice(b"{\"ok\":true}")
        .expect("valid UTF-8 JSON bytes must decode");
    assert_eq!(value, json!({"ok": true}));
}

/// Verifies that decode slice rejects invalid utf8 for byte target.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_slice_rejects_invalid_utf8_for_byte_target() {
    let error = LenientJsonDecoder::new(JsonDecodeOptions::strict())
        .decode_slice::<ByteBuffer>(b"\"\xff\"")
        .expect_err(
            "invalid UTF-8 must be rejected before byte deserialization",
        );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidUtf8);
}

/// Verifies that decode slice invokes target deserializer once on failure.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_slice_invokes_target_deserializer_once_on_failure() {
    reset_deserialize_calls();
    let error = LenientJsonDecoder::new(JsonDecodeOptions::strict())
        .decode_slice::<CountedFailure>(br#""value""#)
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
fn test_decode_slice_accepts_non_rewrite_strict_overrides() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::strict()
            .with_max_input_bytes(Some(64))
            .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    );
    let value: Value = decoder
        .decode_slice(b"{\"ok\":true}")
        .expect("non-rewrite options must preserve successful byte decoding");
    assert_eq!(value, json!({"ok": true}));
}

/// Verifies that decode slice preserves deserialize error mapping.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_slice_preserves_deserialize_error_mapping() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::strict()
            .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    );
    let error = decoder.decode_slice::<Message>(b"{\"text\":7}").expect_err(
        "valid JSON with the wrong field type must fail deserialization",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Detailed);
    assert!(std::error::Error::source(&error).is_some());
}

/// Verifies that decode slice preserves invalid json mapping.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_slice_preserves_invalid_json_mapping() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions::strict());
    let error = decoder
        .decode_slice::<Message>(b"{\"text\":\"broken\"")
        .expect_err("malformed typed JSON must remain an invalid JSON error");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode slice checks raw size before utf8.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_slice_checks_raw_size_before_utf8() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::strict().with_max_input_bytes(Some(1)),
    );
    let error = decoder
        .decode_slice::<Value>(&[0xff, 0xfe])
        .expect_err("raw size must be checked before UTF-8");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InputTooLarge);
}

/// Verifies that decode slice accepts input at exact raw size limit.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_slice_accepts_input_at_exact_raw_size_limit() {
    let input = b"null";
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::strict().with_max_input_bytes(Some(input.len())),
    );

    let value = decoder
        .decode_slice::<Value>(input)
        .expect("input at the exact raw byte limit must be accepted");

    assert_eq!(value, Value::Null);
}

/// Verifies that decode slice classifies invalid utf8.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_slice_classifies_invalid_utf8() {
    let error = LenientJsonDecoder::default()
        .decode_slice::<Value>(&[0xff])
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
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode::<User>("")
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
    let decoder = LenientJsonDecoder::default();
    let message: Message = decoder
        .decode("```json\n{\"text\":\"a\nb\"}\n```")
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
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<User>("[{\"name\":\"alice\",\"age\":30}]")
        .expect_err("top-level array should be rejected by decode_object");
    assert_eq!(error.kind(), JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.stage(), JsonDecodeStage::TopLevelCheck);
    assert_eq!(error.expected_top_level(), Some(JsonTopLevelKind::Object));
    assert_eq!(error.actual_top_level(), Some(JsonTopLevelKind::Array));
}

/// Verifies that decode object reports empty input from normalizer.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_reports_empty_input_from_normalizer() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<User>("")
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
    let decoder = LenientJsonDecoder::default();
    let error = decoder.decode_object::<User>("[").expect_err(
        "malformed JSON should be reported before top-level checking",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode object reports invalid json for malformed scalar.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_reports_invalid_json_for_malformed_scalar() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder.decode_object::<User>("\"unterminated").expect_err(
        "malformed scalar JSON should not be treated as a top-level mismatch",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode array requires array top level.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_requires_array_top_level() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_array::<User>("{\"name\":\"alice\",\"age\":30}")
        .expect_err("top-level object should be rejected by decode_array");
    assert_eq!(error.kind(), JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level(), Some(JsonTopLevelKind::Array));
    assert_eq!(error.actual_top_level(), Some(JsonTopLevelKind::Object));
}

/// Verifies that decode array reports empty input from normalizer.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_reports_empty_input_from_normalizer() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_array::<User>("")
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
    let decoder = LenientJsonDecoder::default();
    let error = decoder.decode_array::<User>("{").expect_err(
        "malformed JSON should be reported before top-level checking",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode object rejects scalar top level.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_rejects_scalar_top_level() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<User>("42")
        .expect_err("top-level scalar should be rejected by decode_object");
    assert_eq!(error.kind(), JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level(), Some(JsonTopLevelKind::Object));
    assert_eq!(error.actual_top_level(), Some(JsonTopLevelKind::Other));
}

/// Verifies that decode array rejects scalar top level.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_rejects_scalar_top_level() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_array::<User>("42")
        .expect_err("top-level scalar should be rejected by decode_array");
    assert_eq!(error.kind(), JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level(), Some(JsonTopLevelKind::Array));
    assert_eq!(error.actual_top_level(), Some(JsonTopLevelKind::Other));
}

/// Verifies that decode array succeeds.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_succeeds() {
    let decoder = LenientJsonDecoder::default();
    let people = decoder
        .decode_array::<User>("[{\"name\":\"alice\",\"age\":30}]")
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
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<User>("{\"name\":\"alice\",\"age\":\"old\"}")
        .expect_err(
            "valid object with wrong field type should return Deserialize",
        );
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
}

/// Verifies that decode array reports deserialize error after top level check.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_reports_deserialize_error_after_top_level_check() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_array::<User>("[{\"name\":\"alice\",\"age\":\"old\"}]")
        .expect_err(
            "valid array with wrong element type should return Deserialize",
        );
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
}

/// Verifies that decode allows generic scalar targets.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_allows_generic_scalar_targets() {
    let decoder = LenientJsonDecoder::default();
    let value: i64 = decoder
        .decode("42")
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
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode::<User>("{")
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
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode::<User>("{\"name\":\"alice\",\"age\":\"old\"}")
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
    let error = LenientJsonDecoder::default()
        .decode::<SingleValue>("{\"value\":\"wrong\",")
        .expect_err(
            "incomplete JSON must take precedence over a field type error",
        );

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
fn test_decode_object_reports_invalid_json_when_data_error_precedes_syntax_error()
 {
    let error = LenientJsonDecoder::default()
        .decode_object::<SingleValue>("{\"value\":\"wrong\",")
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
fn test_decode_array_reports_invalid_json_when_data_error_precedes_syntax_error()
 {
    let error = LenientJsonDecoder::default()
        .decode_array::<u8>("[\"wrong\",")
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
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_trim_whitespace(false)
            .with_markdown_fence_policy(MarkdownFencePolicy::Disabled),
    );
    let error = decoder
        .decode_object::<User>(" \n\t ")
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
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_markdown_fence_policy(MarkdownFencePolicy::Disabled),
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
    let strict_decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_markdown_fence_policy(MarkdownFencePolicy::Disabled),
    );
    let permissive_decoder = LenientJsonDecoder::default();

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
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_trim_whitespace(false),
    );
    let error = decoder
        .decode_value(" \n\t")
        .expect_err("trim disabled should leave whitespace for parser");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode object preserves u128 without value round trip.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_preserves_u128_without_value_round_trip() {
    let decoded: ExactInteger = LenientJsonDecoder::default()
        .decode_object(r#"{"value":340282366920938463463374607431768211455}"#)
        .expect("direct object decoding should preserve u128::MAX");

    assert_eq!(decoded.value, u128::MAX);
}

/// Verifies that decode array preserves u128 without value round trip.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_array_preserves_u128_without_value_round_trip() {
    let decoded: Vec<ExactInteger> = LenientJsonDecoder::default()
        .decode_array(r#"[{"value":340282366920938463463374607431768211455}]"#)
        .expect("direct array decoding should preserve u128::MAX");

    assert_eq!(decoded, vec![ExactInteger { value: u128::MAX }]);
}

/// Verifies that decode object preserves duplicate field rejection.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_object_preserves_duplicate_field_rejection() {
    let error = LenientJsonDecoder::default()
        .decode_object::<SingleValue>(r#"{"value":1,"value":2}"#)
        .expect_err("direct object decoding should reject duplicate fields");

    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
}
