//! J3 JSON refactor contract tests.

mod owner_tests;
mod number_tests;
mod decode_tests;
mod projection_tests;
mod pretty_tests;
mod io_tests;
mod composition_tests;

#[cfg(test)]
mod api_tests {
    use crate::decode::{JsonDecodeNumberModel, JsonValidatedDecodeOptions};

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
}
