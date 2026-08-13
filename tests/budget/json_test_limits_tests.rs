// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Test-only builder for JSON encode sessions.

use qubit_budget::ResourceLimit;
use qubit_json::JsonEncodeLimits;
use qubit_json::JsonEncodeSession;
use qubit_json::JsonResource;
use qubit_json::JsonValueLimits;

/// Builds equivalent directional sessions for integration boundary tests.
#[derive(Clone)]
pub(super) struct JsonTestLimits {
    /// Optional complete-output byte maximum.
    output_bytes: Option<usize>,
    /// Shared direction-independent JSON value limits.
    value: JsonValueLimits,
}

impl JsonTestLimits {
    /// Creates a test configuration with every limit disabled.
    pub(super) const fn new() -> Self {
        Self {
            output_bytes: None,
            value: JsonValueLimits::empty(),
        }
    }

    /// Sets the complete-output byte maximum used by encode sessions.
    pub(super) const fn with_max_output_bytes(mut self, maximum: usize) -> Self {
        self.output_bytes = Some(maximum);
        self
    }

    /// Sets the root-inclusive depth maximum shared by both directions.
    pub(super) fn with_max_depth(mut self, maximum: usize) -> Self {
        let structure = self
            .value
            .structure_limits()
            .with_depth_limit(ResourceLimit::new(JsonResource::Depth, maximum));
        self.value = self.value.with_structure_limits(structure);
        self
    }

    /// Sets the cumulative node maximum shared by both directions.
    pub(super) fn with_max_nodes(mut self, maximum: usize) -> Self {
        let structure = self
            .value
            .structure_limits()
            .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, maximum));
        self.value = self.value.with_structure_limits(structure);
        self
    }

    /// Sets the per-array item maximum shared by both directions.
    pub(super) fn with_max_sequence_items(mut self, maximum: usize) -> Self {
        let structure = self
            .value
            .structure_limits()
            .with_sequence_items_limit(ResourceLimit::new(JsonResource::SequenceItems, maximum));
        self.value = self.value.with_structure_limits(structure);
        self
    }

    /// Sets the per-object entry maximum shared by both directions.
    pub(super) fn with_max_map_entries(mut self, maximum: usize) -> Self {
        let structure = self
            .value
            .structure_limits()
            .with_map_entries_limit(ResourceLimit::new(JsonResource::MapEntries, maximum));
        self.value = self.value.with_structure_limits(structure);
        self
    }

    /// Sets the per-key UTF-8 byte maximum shared by both directions.
    pub(super) fn with_max_key_bytes(mut self, maximum: usize) -> Self {
        let structure = self
            .value
            .structure_limits()
            .with_key_bytes_limit(ResourceLimit::new(JsonResource::KeyBytes, maximum));
        self.value = self.value.with_structure_limits(structure);
        self
    }

    /// Sets the per-string UTF-8 byte maximum shared by both directions.
    pub(super) fn with_max_string_bytes(mut self, maximum: usize) -> Self {
        self.value = self
            .value
            .with_string_bytes_limit(ResourceLimit::new(JsonResource::StringBytes, maximum));
        self
    }

    /// Sets the per-number lexical byte maximum shared by both directions.
    pub(super) fn with_max_number_bytes(mut self, maximum: usize) -> Self {
        self.value = self
            .value
            .with_number_bytes_limit(ResourceLimit::new(JsonResource::NumberBytes, maximum));
        self
    }

    /// Sets the cumulative key, string, and number payload maximum.
    pub(super) fn with_max_payload_bytes(mut self, maximum: usize) -> Self {
        self.value = self
            .value
            .with_payload_bytes_limit(ResourceLimit::new(JsonResource::PayloadBytes, maximum));
        self
    }

    /// Creates fresh encode accounting from this test configuration.
    pub(super) fn encode_session(&self) -> JsonEncodeSession<'static> {
        let mut limits = JsonEncodeLimits::empty().with_value_limits(self.value);
        if let Some(maximum) = self.output_bytes {
            limits = limits
                .with_output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, maximum));
        }
        JsonEncodeSession::owned(limits)
    }
}
