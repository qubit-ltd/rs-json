// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Builder for validated JSON decode configuration.

use super::JsonDecodeNumberModel;
use super::JsonRootKind;
use super::JsonValidatedAdmissionPolicy;
use super::JsonValidatedDecodeOptions;

/// Builds [`JsonValidatedDecodeOptions`].
///
/// Unset optional settings impose no additional limit. A configured zero is a
/// valid bound and permits zero counted nodes or zero source bytes.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::{JsonDecoder, JsonNumberPolicy, JsonValidatedDecodeOptions};
///
/// let options = JsonValidatedDecodeOptions::builder()
///     .max_value_and_key_nodes(3)
///     .max_encoded_string_bytes(8)
///     .build();
/// let value = JsonDecoder::unlimited().decode_unique_validated_str(
///     r#"{"id":42}"#,
///     &options,
///     |_path, _token| Ok::<_, std::convert::Infallible>(JsonNumberPolicy::ExactSafeInteger),
/// )?;
/// assert_eq!(value.as_value()["id"].as_i64(), Some(42));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default)]
pub struct JsonValidatedDecodeOptionsBuilder {
    /// Required root kind, when configured.
    root_kind: Option<JsonRootKind>,
    /// Token admission order for validated decoding.
    admission_policy: JsonValidatedAdmissionPolicy,
    /// Representation model used for preserved numbers.
    number_model: JsonDecodeNumberModel,
    /// Optional combined value-node and object-key-node limit.
    max_value_and_key_nodes: Option<usize>,
    /// Optional per-string-token source-byte limit.
    max_encoded_string_bytes: Option<usize>,
}

impl JsonValidatedDecodeOptionsBuilder {
    /// Selects the token admission order for validated decoding.
    ///
    /// # Parameters
    ///
    /// * `policy` - The admission order to use.
    ///
    /// # Returns
    ///
    /// The builder configured with the selected admission policy.
    pub fn admission_policy(mut self, policy: JsonValidatedAdmissionPolicy) -> Self {
        self.admission_policy = policy;
        self
    }
    /// Requires a particular root kind.
    ///
    /// # Parameters
    ///
    /// * `kind` - Root kind required for a successfully decoded document.
    ///
    /// # Returns
    ///
    /// The builder configured with the required root kind.
    pub fn root_kind(mut self, kind: JsonRootKind) -> Self {
        self.root_kind = Some(kind);
        self
    }
    /// Selects the number representation model.
    ///
    /// # Parameters
    ///
    /// * `model` - Number representation model used when numbers are preserved.
    ///
    /// # Returns
    ///
    /// The builder configured with the selected number model.
    pub fn number_model(mut self, model: JsonDecodeNumberModel) -> Self {
        self.number_model = model;
        self
    }
    /// Limits value entries and decoded object-key entries per document.
    ///
    /// Every JSON value counts once, and every object key also counts once.
    /// Zero is a valid limit and permits no counted entries.
    ///
    /// # Parameters
    ///
    /// * `maximum` - Maximum combined value and object-key node count.
    ///
    /// # Returns
    ///
    /// The builder with this per-document limit configured.
    pub fn max_value_and_key_nodes(mut self, maximum: usize) -> Self {
        self.max_value_and_key_nodes = Some(maximum);
        self
    }
    /// Limits the source bytes inside each string token, excluding quotes.
    /// Escape spelling is counted as it appears in the source, so `\u0061`
    /// consumes six bytes. Zero permits only empty string token contents.
    ///
    /// # Parameters
    ///
    /// * `maximum` - Maximum number of source bytes inside each string token.
    ///
    /// # Returns
    ///
    /// The builder with this per-token limit configured.
    pub fn max_encoded_string_bytes(mut self, maximum: usize) -> Self {
        self.max_encoded_string_bytes = Some(maximum);
        self
    }
    /// Builds the configured options.
    ///
    /// # Returns
    ///
    /// The options snapshot containing all configured settings.
    pub fn build(self) -> JsonValidatedDecodeOptions {
        JsonValidatedDecodeOptions {
            root_kind: self.root_kind,
            admission_policy: self.admission_policy,
            number_model: self.number_model,
            max_value_and_key_nodes: self.max_value_and_key_nodes,
            max_encoded_string_bytes: self.max_encoded_string_bytes,
        }
    }
}
