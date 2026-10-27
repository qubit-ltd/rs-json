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
use super::JsonValidatedDecodeOptionsBuilder;

/// Generic structural and source-token limits for validated decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonValidatedDecodeOptions {
    pub(super) root_kind: Option<JsonRootKind>,
    pub(super) number_model: JsonDecodeNumberModel,
    pub(super) max_value_and_key_nodes: Option<usize>,
    pub(super) max_encoded_string_bytes: Option<usize>,
}

impl JsonValidatedDecodeOptions {
    /// Creates a builder with no extra limits and the `Integer64` model.
    pub fn builder() -> JsonValidatedDecodeOptionsBuilder {
        JsonValidatedDecodeOptionsBuilder::default()
    }
    /// Returns the optional required root kind.
    pub const fn root_kind(&self) -> Option<JsonRootKind> {
        self.root_kind
    }
    /// Returns the selected number model.
    pub const fn number_model(&self) -> JsonDecodeNumberModel {
        self.number_model
    }
    /// Returns the optional per-document value-and-key node limit.
    pub const fn max_value_and_key_nodes(&self) -> Option<usize> {
        self.max_value_and_key_nodes
    }
    /// Returns the optional per-token encoded string byte limit.
    pub const fn max_encoded_string_bytes(&self) -> Option<usize> {
        self.max_encoded_string_bytes
    }
}
