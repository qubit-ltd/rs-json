// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_json::value::SafeJsonValue;

#[test]
fn test_safe_value_bridge_rejects_decoded_duplicate_keys() {
    let error =
        serde_json::from_str::<SafeJsonValue>(r#"{"a":1,"\u0061":2}"#).expect_err("decoded duplicate keys must fail");
    assert!(error.is_data(), "duplicates must retain the data error category");
    assert!(
        error.to_string().contains("duplicate object key"),
        "duplicate diagnostic must stay stable"
    );
}

#[test]
fn test_safe_value_bridge_preserves_all_wide_object_entries() {
    let fields = (0..4096)
        .map(|i| format!("\"key-{i:04}\":{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let text = format!("{{{fields}}}");
    let owner: SafeJsonValue = serde_json::from_str(&text).expect("wide object must decode");
    let object = owner.as_value().as_object().expect("root must remain an object");
    assert_eq!(object.len(), 4096, "all distinct entries must be preserved");
    for i in 0..4096 {
        assert_eq!(object[&format!("key-{i:04}")].as_u64(), Some(i as u64));
    }
}

#[test]
fn test_safe_value_bridge_rejects_duplicates_at_both_object_positions() {
    for input in [r#"{"a":1,"b":2,"a":3}"#, r#"{"b":1,"a":2,"a":3}"#] {
        let error = serde_json::from_str::<SafeJsonValue>(input).expect_err("repeated key must fail");
        assert!(error.is_data(), "duplicates must retain the data error category");
        assert!(
            error.to_string().contains("duplicate object key"),
            "duplicate diagnostic must stay stable"
        );
    }
}

#[test]
fn test_safe_value_bridge_preserves_object_entry_order() {
    let input = r#"{"z":1,"a":2,"m":3}"#;
    let expected: serde_json::Value = serde_json::from_str(input).expect("reference object must decode");
    let owner: SafeJsonValue = serde_json::from_str(input).expect("unique object must decode");
    let object = owner.as_value().as_object().expect("root must remain an object");
    let expected = expected.as_object().expect("reference root must be an object");
    assert_eq!(
        object.keys().map(String::as_str).collect::<Vec<_>>(),
        expected.keys().map(String::as_str).collect::<Vec<_>>(),
    );
}
// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
