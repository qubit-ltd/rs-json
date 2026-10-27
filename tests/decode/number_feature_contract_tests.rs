// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_budget::ResourceLimit;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json::encode::JsonEncodeErrorKind;
use qubit_json::encode::JsonEncoder;
use qubit_json::value::AccountingJsonValueSeed;
use qubit_json::value::DuplicateKeyRejectingJsonValue;
use qubit_json::value::DuplicateKeyRejectingJsonValueSeed;
use qubit_json::value::JsonValueEncoder;
use qubit_json::value::traverse::JsonTreeBudgetTracker;
use serde::Serialize;
use serde::Serializer;
use serde::de::DeserializeSeed;
use serde::de::value::I128Deserializer;
use serde::de::value::U128Deserializer;
use serde::ser::SerializeMap;
use serde_json::Number;
use serde_json::Value;
use serde_json::from_value;
use serde_json::to_vec;

/// Emits a single scalar map key for cross-encoder contract checks.
struct FloatKeyProbe<T>(T);

#[test]
fn test_unique_key_values_preserve_numbers_across_feature_unification() {
    for (text, expected) in [
        ("1.5", 1.5_f64),
        ("1e2", 100.0),
        ("-0", -0.0),
        ("0e+10", 0.0),
        ("-1e-400", -0.0),
        ("5e-324", f64::from_bits(1)),
    ] {
        let value: DuplicateKeyRejectingJsonValue = JsonDecoder::unlimited()
            .decode_unique_str(text)
            .expect("a finite number must decode");
        assert_eq!(
            value.into_inner().as_f64().map(f64::to_bits),
            Some(expected.to_bits()),
            "number must retain its type and sign: {text}",
        );
    }
}

#[test]
fn test_unique_decoder_preserves_integer_union_and_rejects_overflow() {
    let unsigned: DuplicateKeyRejectingJsonValue = JsonDecoder::unlimited()
        .decode_unique_str("18446744073709551615")
        .expect("u64 maximum must remain exact");
    assert_eq!(unsigned.into_inner().as_u64(), Some(u64::MAX));
    let signed: DuplicateKeyRejectingJsonValue = JsonDecoder::unlimited()
        .decode_unique_str("-9223372036854775808")
        .expect("i64 minimum must remain exact");
    assert_eq!(signed.into_inner().as_i64(), Some(i64::MIN));
    for text in ["18446744073709551616", "-9223372036854775809", "1e400"] {
        let error = JsonDecoder::unlimited()
            .decode_unique_str::<DuplicateKeyRejectingJsonValue>(text)
            .expect_err("wide numbers must not bypass strict source admission");
        assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    }
}

#[test]
fn test_unique_key_values_preserve_nested_numbers_and_marker_objects() {
    let text = r#"{"amount":1.5,"items":[1e2,-0],"marker":{"$serde_json::private::Number":"1.5"}}"#;
    let value = JsonDecoder::unlimited()
        .decode_unique_seed_utf8(DuplicateKeyRejectingJsonValueSeed::new(), text.as_bytes())
        .expect("nested numbers and marker objects must decode")
        .into_inner();
    assert_eq!(value["amount"].as_f64(), Some(1.5));
    assert_eq!(value["items"][0].as_f64(), Some(100.0));
    assert_eq!(value["items"][1].as_f64().map(f64::to_bits), Some((-0.0_f64).to_bits()));
    assert_eq!(value["marker"]["$serde_json::private::Number"], "1.5");
    assert!(
        JsonDecoder::unlimited()
            .decode_unique_str::<DuplicateKeyRejectingJsonValue>(r#"{"amount":1.5,"amount":2.5}"#)
            .is_err()
    );
}

#[test]
fn test_unique_key_values_preserve_raw_marker_objects() {
    let text = r#"{"$serde_json::private::RawValue":"[1.5]","other":1.5}"#;
    let value: DuplicateKeyRejectingJsonValue = JsonDecoder::unlimited()
        .decode_unique_str(text)
        .expect("source marker-named fields must remain literal");
    let value = value.into_inner();
    assert_eq!(value["$serde_json::private::RawValue"], "[1.5]");
    assert_eq!(value["other"].as_f64(), Some(1.5));
    let decoded: DuplicateKeyRejectingJsonValue = JsonDecoder::unlimited()
        .decode_unique_utf8(&to_vec(&value).unwrap())
        .expect("re-encoded marker objects must remain literal");
    assert_eq!(decoded.into_inner(), value);
}

#[test]
fn test_unique_key_values_reject_nested_decoded_duplicate_keys() {
    let error = JsonDecoder::unlimited()
        .with_diagnostic_policy(qubit_json::decode::DiagnosticPolicy::Detailed)
        .decode_unique_str::<DuplicateKeyRejectingJsonValue>(r#"{"items":[{"amount":1.5,"\u0061mount":2.5}]}"#)
        .expect_err("decoded duplicate keys must still be rejected");
    assert!(error.to_string().contains("duplicate JSON object key"));
    assert_eq!(error.line(), Some(1));
    assert!(error.column().is_some_and(|column| column > 0));
}

#[test]
fn test_unique_decoder_supports_typed_containers_and_enum() {
    let value: std::collections::BTreeMap<u64, Vec<Option<f64>>> = JsonDecoder::unlimited()
        .decode_unique_str(r#"{"42":[1.5,null,-0]}"#)
        .expect("typed map keys and options must decode");
    assert_eq!(value[&42][0], Some(1.5));
    assert_eq!(value[&42][1], None);
    assert_eq!(value[&42][2].map(f64::to_bits), Some((-0.0_f64).to_bits()));
    let value: Result<(f64, bool), String> = JsonDecoder::unlimited()
        .decode_unique_str(r#"{"Ok":[1.5,true]}"#)
        .expect("externally tagged enums must decode");
    assert_eq!(value, Ok((1.5, true)));
}

impl<T: Serialize> Serialize for FloatKeyProbe<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.0, &true)?;
        map.end()
    }
}

#[test]
fn test_real_number_zero_exponents_and_underflow() {
    for (text, expected) in [
        ("0e1", 0.0_f64),
        ("0e+10", 0.0),
        ("0.0e-10", 0.0),
        ("-0e1", -0.0),
        ("-0.0e+10", -0.0),
        ("1e-400", 0.0),
        ("-1e-400", -0.0),
        ("5e-324", f64::from_bits(1)),
        ("-5e-324", -f64::from_bits(1)),
    ] {
        let number: Number = serde_json::from_str(text).expect(text);
        let value = JsonValueEncoder::new().encode(&number).expect(text);
        assert_eq!(value.as_f64().expect(text).to_bits(), expected.to_bits(), "{text}");

        let bytes = JsonEncoder::unlimited().to_vec(&number).expect(text);
        let decoded: Value = JsonDecoder::unlimited().decode_utf8(&bytes).expect(text);
        assert_eq!(decoded.as_f64().expect(text).to_bits(), expected.to_bits(), "{text}");
    }
}

#[test]
fn test_json_value_encoder_f32_keys_match_direct_encoding() {
    for key in [
        1.0_f32,
        -1.0,
        0.0,
        -0.0,
        1.5,
        1e20,
        -1e20,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::MAX,
    ] {
        let probe = FloatKeyProbe(key);
        let projected = JsonValueEncoder::new().encode(&probe).unwrap();
        let direct = JsonEncoder::unlimited().to_vec(&probe).unwrap();
        assert_eq!(to_vec(&projected).unwrap(), direct, "key bits={:08x}", key.to_bits());
    }
}

#[test]
fn test_json_value_encoder_f32_values_remain_strictly_encodable() {
    for source in [
        1.0_f32,
        -1.0,
        0.0,
        -0.0,
        1.5,
        1e20,
        -1e20,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::MAX,
        f32::from_bits(0xC65B_9806),
    ] {
        let direct = JsonEncoder::unlimited().to_vec(&source).unwrap();
        let expected: Value = JsonDecoder::unlimited().decode_utf8(&direct).unwrap();
        let projected = JsonValueEncoder::new().encode(&source).unwrap();
        assert_eq!(projected, expected, "value bits={:08x}", source.to_bits());
        let encoded = JsonEncoder::unlimited().to_vec(&projected).unwrap();
        let _: Value = JsonDecoder::unlimited().decode_utf8(&encoded).unwrap();
        assert_eq!(
            from_value::<f32>(projected).unwrap().to_bits(),
            source.to_bits(),
            "f32 round trip must retain the original bits",
        );
    }
}

#[test]
fn test_tree_number_measurement_uses_the_stored_representation() {
    for text in ["-0", "1.5", "18446744073709551615"] {
        let value: Value = serde_json::from_str(text).unwrap();
        let expected = serde_json::to_vec(value.as_number().unwrap()).unwrap().len();
        let limits = JsonValueLimits::<JsonResource, usize>::builder()
            .max_number_bytes(expected)
            .build();
        JsonTreeBudgetTracker::new(limits)
            .account(&value)
            .expect("the exact stored number representation should fit");
    }
}

#[test]
fn test_value_encoder_keeps_marker_named_objects_as_objects() {
    let value = JsonValueEncoder::new()
        .encode(&serde_json::json!({"$serde_json::private::Number": "1.5"}))
        .unwrap();
    assert!(value.is_object());
}

#[test]
fn test_private_raw_key_obeys_serde_value_target_semantics() {
    let input = r#"{"$serde_json::private::RawValue":"null"}"#;
    let value: Value = JsonDecoder::unlimited().decode_str(input).unwrap();
    assert!(value.is_null());

    let error = JsonDecoder::unlimited()
        .decode_str::<Value>(r#"{"$serde_json::private::RawValue":8}"#)
        .unwrap_err();
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
}

#[test]
fn test_private_raw_key_preserves_source_accounting_and_root_check() {
    let input = r#"{"$serde_json::private::RawValue":"[1,2,3]"}"#;
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder().max_nodes(2).build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let value: Value = decoder.decode_object_str(input).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 3);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(2));
}

#[test]
fn test_private_raw_key_can_be_literal_with_unique_key_target() {
    for input in [
        r#"{"$serde_json::private::RawValue":"null"}"#,
        r#"{"$serde_json::private::RawValue":"[1,2,3]"}"#,
    ] {
        let value: DuplicateKeyRejectingJsonValue = JsonDecoder::unlimited().decode_utf8(input.as_bytes()).unwrap();
        let value = value.into_inner();
        assert!(value.is_object());
        assert!(value["$serde_json::private::RawValue"].is_string());
    }
}

#[test]
fn test_seed_entry_points_enforce_the_64_bit_integer_union() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::default());
    let mut transaction = budget.transaction();
    let accepted = AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(U128Deserializer::<serde_json::Error>::new(u64::MAX as u128))
        .unwrap();
    assert_eq!(accepted.as_u64(), Some(u64::MAX));
    assert!(
        AccountingJsonValueSeed::new(&mut transaction)
            .deserialize(U128Deserializer::<serde_json::Error>::new(u128::MAX))
            .is_err()
    );

    assert!(
        DuplicateKeyRejectingJsonValueSeed::new()
            .deserialize(I128Deserializer::<serde_json::Error>::new(i64::MIN as i128 - 1))
            .is_err()
    );
}

#[test]
fn test_arbitrary_precision_tree_numbers_are_counted_without_float_conversion() {
    let texts: &[&str] = if supports_arbitrary_precision() {
        &["340282366920938463463374607431768211455", "1e400"]
    } else {
        &["340282366920938463463374607431768211455"]
    };
    for text in texts {
        let value: Value = serde_json::from_str(text).unwrap();
        let expected = serde_json::to_vec(value.as_number().unwrap()).unwrap().len();
        let exact_limits = JsonValueLimits::<JsonResource, usize>::builder()
            .max_number_bytes(expected)
            .max_nodes(4)
            .build();
        JsonTreeBudgetTracker::new(exact_limits)
            .account(&value)
            .expect("the stored number representation should fit its exact byte limit");

        let limited = expected - 1;
        let limits = JsonValueLimits::<JsonResource, usize>::builder()
            .max_number_bytes(limited)
            .max_nodes(4)
            .build();
        let mut tracker = JsonTreeBudgetTracker::new(limits);
        let error = tracker.account(&value).unwrap_err();
        assert_eq!(error.resource(), &JsonResource::NumberBytes);
        assert_eq!(
            error
                .budget_error()
                .expect("number-byte rejection is a budget limit error")
                .exact_observed(),
            Some(expected)
        );
        assert_eq!(tracker.budget().used_nodes(), Some(0));
    }
}

#[test]
fn test_arbitrary_precision_real_numbers_still_enforce_strict_ranges() {
    if !supports_arbitrary_precision() {
        for text in ["1e400", "18446744073709551616"] {
            assert_eq!(
                JsonDecoder::unlimited()
                    .decode_unique_str::<DuplicateKeyRejectingJsonValue>(text)
                    .unwrap_err()
                    .kind(),
                JsonDecodeErrorKind::InvalidJson,
                "{text}"
            );
        }
        return;
    }
    for text in ["1e400", "18446744073709551616"] {
        let number: Number = serde_json::from_str(text).expect("arbitrary-precision Number must parse");
        let error = JsonEncoder::unlimited()
            .to_vec(&number)
            .expect_err("ordinary encoding must reject values outside the strict range");
        assert_eq!(error.kind(), JsonEncodeErrorKind::Serialize, "{text}");
    }
}

#[test]
fn test_arbitrary_precision_real_number_bytes_are_budgeted() {
    let number: Number = serde_json::from_str("0e10").expect("the number must parse");
    let serialized_len = serde_json::to_vec(&number).unwrap().len();
    let limits = JsonEncodeLimits::builder()
        .output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 256_usize))
        .value_limits(
            JsonValueLimits::builder()
                .max_number_bytes(serialized_len - 1)
                .max_nodes(64_usize)
                .build(),
        )
        .build();
    let mut encoder = JsonEncoder::new(JsonEncodeSession::from_limits(limits));
    let error = encoder
        .to_vec(&number)
        .expect_err("the stored number representation must exceed its byte budget");
    assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
}

/// Detects the actual serde_json Number model selected by unified dependency
/// features.
fn supports_arbitrary_precision() -> bool {
    match serde_json::from_str::<Value>("1e400") {
        Ok(value) => {
            assert_eq!(value.as_number().unwrap().to_string(), "1e+400");
            true
        }
        Err(error) => {
            assert!(error.is_syntax());
            false
        }
    }
}
