// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_json::decode::JsonDecodeNumberModel;
use qubit_json::decode::JsonValidatedAdmissionPolicy;
use qubit_json::decode::JsonValidatedDecodeOptions;

#[test]
fn test_admission_policy_is_explicit_and_defaults_to_immediate() {
    let defaults = JsonValidatedDecodeOptions::builder().build();
    assert_eq!(defaults.admission_policy(), JsonValidatedAdmissionPolicy::Immediate);
    assert_eq!(defaults.root_kind(), None);
    assert_eq!(defaults.number_model(), JsonDecodeNumberModel::Integer64);
    assert_eq!(defaults.max_value_and_key_nodes(), None);
    assert_eq!(defaults.max_encoded_string_bytes(), None);
    let token_first = JsonValidatedDecodeOptions::builder()
        .admission_policy(JsonValidatedAdmissionPolicy::TokenFirst)
        .max_encoded_string_bytes(0)
        .max_value_and_key_nodes(0)
        .build();
    assert_eq!(token_first.admission_policy(), JsonValidatedAdmissionPolicy::TokenFirst);
    assert_eq!(token_first.max_encoded_string_bytes(), Some(0));
    assert_eq!(token_first.max_value_and_key_nodes(), Some(0));
    assert_eq!(token_first.clone(), token_first);
    assert_ne!(defaults, token_first);
}
