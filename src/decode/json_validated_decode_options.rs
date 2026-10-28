// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Validated JSON decode configuration.

use super::JsonDecodeNumberModel;
use super::JsonRootKind;
use super::JsonValidatedAdmissionPolicy;
use super::JsonValidatedDecodeOptionsBuilder;

/// Generic structural and source-token limits for validated decoding.
///
/// By default, no additional limits are imposed and the number model is
/// `Integer64`. A limit set to `Some(0)` is a real zero-item/zero-byte limit;
/// `None` means that the corresponding additional limit is disabled. Value
/// nodes and object-key nodes both count toward the node limit. String limits
/// count the source spelling inside each token, excluding quotes and
/// including escape bytes such as the six bytes in `\u0061`.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonValidatedDecodeOptions {
    /// Required root kind, when the caller configured one.
    pub(super) root_kind: Option<JsonRootKind>,
    /// Token admission order for validated decoding.
    pub(super) admission_policy: JsonValidatedAdmissionPolicy,
    /// Representation model used for numbers selected for preservation.
    pub(super) number_model: JsonDecodeNumberModel,
    /// Optional combined count for value nodes and object-key nodes.
    pub(super) max_value_and_key_nodes: Option<usize>,
    /// Optional maximum source bytes inside each string token.
    pub(super) max_encoded_string_bytes: Option<usize>,
}

impl JsonValidatedDecodeOptions {
    /// Creates a builder with no extra limits and the `Integer64` model.
    ///
    /// # Returns
    ///
    /// A builder whose optional root kind and additional limits are unset.
    pub fn builder() -> JsonValidatedDecodeOptionsBuilder {
        JsonValidatedDecodeOptionsBuilder::default()
    }
    /// Returns the optional required root kind. `None` accepts any root kind.
    ///
    /// # Returns
    ///
    /// The required root kind, or `None` when no root-kind restriction applies.
    pub const fn root_kind(&self) -> Option<JsonRootKind> {
        self.root_kind
    }
    /// Returns the token admission order used by validated decoding.
    ///
    /// # Returns
    ///
    /// The configured policy. The default is
    /// [`JsonValidatedAdmissionPolicy::Immediate`].
    pub const fn admission_policy(&self) -> JsonValidatedAdmissionPolicy {
        self.admission_policy
    }
    /// Returns the selected number model.
    ///
    /// # Returns
    ///
    /// The number representation model used for preserved numbers.
    pub const fn number_model(&self) -> JsonDecodeNumberModel {
        self.number_model
    }
    /// Returns the optional per-document value-and-key node limit. `None`
    /// disables this additional limit; `Some(0)` permits no nodes.
    ///
    /// # Returns
    ///
    /// The configured combined node limit, or `None` when it is disabled.
    pub const fn max_value_and_key_nodes(&self) -> Option<usize> {
        self.max_value_and_key_nodes
    }
    /// Returns the optional per-token source-string byte limit. It excludes
    /// the quotes and includes escape spelling bytes. `None` disables this
    /// additional limit; `Some(0)` permits only empty string token contents.
    ///
    /// # Returns
    ///
    /// The configured per-token source-byte limit, or `None` when disabled.
    pub const fn max_encoded_string_bytes(&self) -> Option<usize> {
        self.max_encoded_string_bytes
    }
}
