// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! J3 JSON refactor contract tests.

mod composition_tests;
mod decode_tests;
mod io_tests;
mod number_tests;
mod owner_tests;
mod pretty_tests;
mod projection_tests;

#[cfg(test)]
mod api_tests {
    use qubit_budget::json::JsonResource;
    use qubit_budget::json::JsonValueBudget;
    use qubit_budget::json::JsonValueLimits;

    use crate::decode::JsonDecodeNumberModel;
    use crate::decode::JsonValidatedDecodeOptions;
    use crate::encode::JsonSerializationErrorKind;
    use crate::value::JsonValueEncoder;

    #[test]
    fn test_validated_options_keep_zero_and_defaults() {
        let default = JsonValidatedDecodeOptions::builder().build();
        assert_eq!(default.root_kind(), None);
        assert_eq!(default.number_model(), JsonDecodeNumberModel::Integer64);
        assert_eq!(default.max_value_and_key_nodes(), None);
        assert_eq!(default.max_encoded_string_bytes(), None);

        let zero = JsonValidatedDecodeOptions::builder()
            .max_value_and_key_nodes(0)
            .max_encoded_string_bytes(0)
            .build();
        assert_eq!(zero.max_value_and_key_nodes(), Some(0));
        assert_eq!(zero.max_encoded_string_bytes(), Some(0));
    }

    #[test]
    fn test_value_encode_error_accessors_cover_serialization_and_budget() {
        let serialization = {
            let mut unlimited = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
            let mut transaction = unlimited.transaction();
            JsonValueEncoder::new()
                .encode_in_transaction(&u128::MAX, &mut transaction)
                .unwrap_err()
        };
        assert_eq!(
            serialization.kind(),
            Some(JsonSerializationErrorKind::IntegerOutOfRange {
                signedness: crate::encode::JsonIntegerSignedness::Unsigned,
            })
        );
        assert_eq!(
            serialization.category(),
            Some(crate::encode::JsonSerializationErrorCategory::Number)
        );
        assert!(serialization.serialization_error().is_some());
        assert!(serialization.is_number_error());
        assert!(!serialization.is_map_key_error());
        assert!(!serialization.is_raw_value_error());
        assert!(!serialization.is_serializer_contract_error());
        assert_eq!(
            serialization.integer_signedness(),
            Some(crate::encode::JsonIntegerSignedness::Unsigned)
        );
        assert_eq!(serialization.map_key_kind(), None);
        assert_eq!(serialization.collection_kind(), None);
        assert_eq!(serialization.serializer_state_error(), None);
        assert!(std::error::Error::source(&serialization).is_some());
        assert!(serialization.to_string().contains("64-bit"));
        let limits = JsonValueLimits::<JsonResource, usize>::builder().max_nodes(0).build();
        let mut budget = JsonValueBudget::new(limits);
        let mut transaction = budget.transaction();
        let rejected = JsonValueEncoder::new()
            .encode_in_transaction(&true, &mut transaction)
            .unwrap_err();
        assert!(rejected.serialization_error().is_none());
        assert_eq!(rejected.kind(), None);
        assert_eq!(rejected.category(), None);
        assert!(!rejected.is_number_error());
        assert!(!rejected.is_map_key_error());
        assert!(!rejected.is_raw_value_error());
        assert!(!rejected.is_serializer_contract_error());
        assert_eq!(rejected.integer_signedness(), None);
        assert_eq!(rejected.map_key_kind(), None);
        assert_eq!(rejected.collection_kind(), None);
        assert_eq!(rejected.serializer_state_error(), None);
        assert!(std::error::Error::source(&rejected).is_some());
        assert!(rejected.to_string().contains("budget rejected"));
    }
}
