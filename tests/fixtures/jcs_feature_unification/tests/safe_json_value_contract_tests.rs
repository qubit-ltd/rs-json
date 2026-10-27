// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_json::decode::JsonDecoder;
use qubit_json::value::SafeJsonValue;
fn assert_bridge_number(text: &str, ordinary: &str, _precise: Option<&str>) {
    let owner: SafeJsonValue = serde_json::from_str(text).expect("JSON number must deserialize");
    #[cfg(not(feature = "arbitrary_precision"))]
    {
        assert!(owner.as_value().is_number(), "{text} must remain a number");
        assert_eq!(
            serde_json::to_string(&owner).expect("serialize number"),
            ordinary
        );
    }
    #[cfg(feature = "arbitrary_precision")]
    {
        if let Some(precise) = _precise {
            assert_eq!(
                owner.as_value()["$serde_json::private::Number"].as_str(),
                Some(precise),
                "{text} must retain serde_json's arbitrary-precision bridge map"
            );
            assert_eq!(
                serde_json::to_string(&owner).expect("serialize bridge map"),
                format!(r#"{{"$serde_json::private::Number":"{precise}"}}"#)
            );
        } else {
            assert!(owner.as_value().is_number(), "{text} must remain a number");
            assert_eq!(
                serde_json::to_string(&owner).expect("serialize integer"),
                ordinary
            );
        }
    }
}

#[test]
fn test_safe_value_bridge_has_explicit_feature_semantics() {
    assert_bridge_number("1.25", "1.25", Some("1.25"));
    assert_bridge_number("7", "7", None);
    assert_bridge_number("1e2", "100.0", Some("1e+2"));
    assert_bridge_number("-0", "-0.0", Some("-0"));

    let owner: SafeJsonValue = serde_json::from_str(r#"{"nested":[1.25,{"x":-0}]}"#)
        .expect("nested JSON must deserialize");
    let nested = owner.as_value().as_object().expect("root object");
    let array = nested["nested"].as_array().expect("nested array");
    #[cfg(not(feature = "arbitrary_precision"))]
    {
        assert_eq!(array[0].as_f64(), Some(1.25));
        assert_eq!(array[1]["x"].as_f64(), Some(-0.0));
        assert!(array[1]["x"].as_f64().is_some_and(f64::is_sign_negative));
    }
    #[cfg(feature = "arbitrary_precision")]
    {
        assert_eq!(
            array[0]["$serde_json::private::Number"].as_str(),
            Some("1.25")
        );
        assert_eq!(
            array[1]["x"]["$serde_json::private::Number"].as_str(),
            Some("-0")
        );
        let owner: SafeJsonValue = serde_json::from_str("1e400")
            .expect("arbitrary precision must preserve out-of-range numbers");
        assert_eq!(
            owner.as_value()["$serde_json::private::Number"].as_str(),
            Some("1e+400")
        );
    }

    let literal: SafeJsonValue = serde_json::from_str(r#"{"$serde_json::private::Number":"1.25"}"#)
        .expect("private-looking input remains an ordinary map");
    assert_eq!(
        literal.as_value(),
        &serde_json::json!({"$serde_json::private::Number": "1.25"})
    );
}

#[test]
fn test_text_aware_decoder_preserves_numeric_meaning_in_each_feature_mode() {
    let protected = JsonDecoder::unlimited()
        .decode_value_str("1.25")
        .expect("text-aware path must preserve numeric meaning");
    assert!(protected.as_value().is_number());
    assert_eq!(protected.as_value().as_f64(), Some(1.25));
    assert_eq!(
        serde_json::to_string(&protected).expect("serialize number"),
        "1.25"
    );

    let nested = JsonDecoder::unlimited()
        .decode_value_str(r#"{"nested":[1.25,1e2,-0]}"#)
        .expect("nested text numbers must retain numeric types");
    let values = nested.as_value()["nested"]
        .as_array()
        .expect("nested array");
    assert_eq!(values[0].as_f64(), Some(1.25));
    assert_eq!(values[1].as_f64(), Some(100.0));
    assert_eq!(values[2].as_f64(), Some(-0.0));
    assert!(values[2].as_f64().is_some_and(f64::is_sign_negative));
}
// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
