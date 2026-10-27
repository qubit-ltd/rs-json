//! Builder for validated JSON decode configuration.

use super::JsonDecodeNumberModel;
use super::JsonRootKind;
use super::JsonValidatedDecodeOptions;

/// Builds [`JsonValidatedDecodeOptions`].
#[derive(Debug, Clone, Default)]
pub struct JsonValidatedDecodeOptionsBuilder {
    root_kind: Option<JsonRootKind>,
    number_model: JsonDecodeNumberModel,
    max_value_and_key_nodes: Option<usize>,
    max_encoded_string_bytes: Option<usize>,
}

impl JsonValidatedDecodeOptionsBuilder {
    /// Requires a particular root kind.
    pub fn root_kind(mut self, kind: JsonRootKind) -> Self { self.root_kind = Some(kind); self }
    /// Selects the number representation model.
    pub fn number_model(mut self, model: JsonDecodeNumberModel) -> Self { self.number_model = model; self }
    /// Limits value entries and decoded object-key entries per document.
    pub fn max_value_and_key_nodes(mut self, maximum: usize) -> Self { self.max_value_and_key_nodes = Some(maximum); self }
    /// Limits the source bytes inside each string token, excluding quotes.
    pub fn max_encoded_string_bytes(mut self, maximum: usize) -> Self { self.max_encoded_string_bytes = Some(maximum); self }
    /// Builds the configured options.
    pub fn build(self) -> JsonValidatedDecodeOptions {
        JsonValidatedDecodeOptions {
            root_kind: self.root_kind,
            number_model: self.number_model,
            max_value_and_key_nodes: self.max_value_and_key_nodes,
            max_encoded_string_bytes: self.max_encoded_string_bytes,
        }
    }
}
