// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Test-only builder for JSON encode sessions.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeLimitsBuilder;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;

/// Builds equivalent directional sessions for integration boundary tests.
#[derive(Clone)]
pub(crate) struct JsonTestLimits {
    /// Complete encode limits used by the test session.
    limits: JsonEncodeLimitsBuilder,
}

impl JsonTestLimits {
    /// Creates a test configuration with every limit disabled.
    pub(crate) fn new() -> Self {
        Self {
            limits: JsonEncodeLimits::<JsonResource, usize>::builder(),
        }
    }

    /// Sets the complete-output byte maximum used by encode sessions.
    pub(crate) fn with_max_output_bytes(mut self, maximum: usize) -> Self {
        self.limits = self.limits.max_output_bytes(maximum);
        self
    }

    /// Sets the root-inclusive depth maximum shared by both directions.
    pub(crate) fn with_max_depth(mut self, maximum: usize) -> Self {
        self.limits = self.limits.max_depth(maximum);
        self
    }

    /// Sets the cumulative node maximum shared by both directions.
    pub(crate) fn with_max_nodes(mut self, maximum: usize) -> Self {
        self.limits = self.limits.max_nodes(maximum);
        self
    }

    /// Sets the per-array item maximum shared by both directions.
    pub(crate) fn with_max_sequence_items(mut self, maximum: usize) -> Self {
        self.limits = self.limits.max_sequence_items(maximum);
        self
    }

    /// Sets the per-object entry maximum shared by both directions.
    pub(crate) fn with_max_map_entries(mut self, maximum: usize) -> Self {
        self.limits = self.limits.max_map_entries(maximum);
        self
    }

    /// Sets the per-key UTF-8 byte maximum shared by both directions.
    pub(crate) fn with_max_key_bytes(mut self, maximum: usize) -> Self {
        self.limits = self.limits.max_key_bytes(maximum);
        self
    }

    /// Sets the per-string UTF-8 byte maximum shared by both directions.
    pub(crate) fn with_max_string_bytes(mut self, maximum: usize) -> Self {
        self.limits = self.limits.max_string_bytes(maximum);
        self
    }

    /// Sets the per-number lexical byte maximum shared by both directions.
    pub(crate) fn with_max_number_bytes(mut self, maximum: usize) -> Self {
        self.limits = self.limits.max_number_bytes(maximum);
        self
    }

    /// Sets the cumulative key, string, and number payload maximum.
    pub(crate) fn with_max_payload_bytes(mut self, maximum: usize) -> Self {
        self.limits = self.limits.max_payload_bytes(maximum);
        self
    }

    /// Creates fresh encode accounting from this test configuration.
    pub(crate) fn encode_session(&self) -> JsonEncodeSession<'static> {
        JsonEncodeSession::owned(self.limits.build())
    }
}
