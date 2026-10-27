// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Cross-API composition and compatibility contract tests.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::decode::JsonDecodeNumberModel;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonValidatedDecodeError;
use qubit_json::decode::JsonValidatedDecodeOptions;
use qubit_json::encode::JsonEncoder;
use qubit_json::format::JsonPrettyOptions;
use qubit_json::format::format_json_tokens;
use qubit_json::value::JsonValueEncoder;
#[cfg(feature = "jcs")]
use qubit_json::value::SafeJsonValue;
use serde_json::Value;
use serde_json::json;

#[test]
fn test_failed_policy_keeps_input_charge_and_next_document_commits_exact_values() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(9)
        .max_nodes(2)
        .max_payload_bytes(2)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let options = JsonValidatedDecodeOptions::builder().build();

    let failed = decoder
        .decode_unique_validated_str("[1]", &options, |_, _| Err("policy failure"))
        .unwrap_err();
    assert!(matches!(failed, JsonValidatedDecodeError::Policy { .. }));
    assert_eq!(decoder.session().input_budget().unwrap().used(), 3);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(decoder.session().value_budget().used_payload_bytes(), Some(0));

    let owner = decoder.decode_value_str("[\"é\"]").unwrap();
    assert_eq!(owner.as_value(), &json!(["é"]));
    assert_eq!(decoder.session().input_budget().unwrap().used(), 9);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(2));
    assert_eq!(decoder.session().value_budget().used_payload_bytes(), Some(2));
}

#[test]
fn test_typed_fallback_keeps_duplicate_map_semantics() {
    let mut input = std::io::Cursor::new(br#"{"wide":18446744073709551616,"x":1,"x":2}"#.to_vec());
    let value: Value = JsonDecoder::unlimited()
        .decode_input_with_number_model(&mut input, JsonDecodeNumberModel::Integer64WithFiniteFallback)
        .unwrap();
    assert_eq!(value["x"], json!(2));
    assert!(value["wide"].as_f64().unwrap().is_finite());
}

#[test]
fn test_guarded_value_composes_with_encoder_and_token_formatter() {
    let source = json!({"ok": true, "items": [1, 2]});
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::builder().max_nodes(5).build());
    let mut transaction = budget.transaction();
    let owner = JsonValueEncoder::new()
        .encode_in_transaction(&source, &mut transaction)
        .unwrap();
    transaction.commit().unwrap();
    assert_eq!(budget.used_nodes(), Some(5));

    let encoded = JsonEncoder::unlimited().to_vec(&owner).unwrap();
    let options = JsonPrettyOptions::builder().build().unwrap();
    let pretty = format_json_tokens(&encoded, &options, None).unwrap();
    let reparsed = JsonDecoder::unlimited().decode_value_utf8(&pretty).unwrap();
    assert_eq!(owner, reparsed);
}

#[test]
fn test_value_decoder_owner_composes_with_text_encoder_and_formatter() {
    let owner = JsonDecoder::unlimited()
        .decode_value_str(r#"{"ok":true,"items":[1,2]}"#)
        .unwrap();
    let encoded = JsonEncoder::unlimited().to_vec(&owner).unwrap();
    let options = JsonPrettyOptions::builder().build().unwrap();
    let pretty = format_json_tokens(&encoded, &options, None).unwrap();
    let reparsed = JsonDecoder::unlimited().decode_value_utf8(&pretty).unwrap();
    assert!(owner == reparsed);
}

#[cfg(feature = "jcs")]
#[test]
fn test_ordinary_integer_jcs_binary64_and_pretty_tokens_remain_separate() {
    let ordinary = JsonDecoder::unlimited().decode_value_str("9007199254740993").unwrap();
    assert_eq!(ordinary.as_value().as_u64(), Some(9_007_199_254_740_993));

    let jcs = qubit_json::decode::JcsDecoder::unlimited()
        .decode_str("9007199254740993")
        .unwrap();
    assert_eq!(jcs.as_f64(), Some(9_007_199_254_740_992.0));
    let jcs = SafeJsonValue::new(jcs);
    assert!(jcs.as_value().is_number());

    let options = JsonPrettyOptions::builder().build().unwrap();
    assert_eq!(
        format_json_tokens(br#"{"n":1e0}"#, &options, None).unwrap(),
        b"{\n  \"n\": 1e0\n}"
    );
}
