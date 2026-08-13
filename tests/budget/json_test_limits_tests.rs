// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Test-only builder for JSON encode sessions.

use qubit_json::JsonEncodeLimits;
use qubit_json::JsonEncodeSession;

/// Builds equivalent directional sessions for integration boundary tests.
#[derive(Clone)]
pub(super) struct JsonTestLimits {
    /// Complete encode limits used by the test session.
    limits: JsonEncodeLimits,
}

impl JsonTestLimits {
    /// Creates a test configuration with every limit disabled.
    pub(super) fn new() -> Self {
        Self {
            limits: JsonEncodeLimits::empty(),
        }
    }

    /// Sets the complete-output byte maximum used by encode sessions.
    pub(super) fn with_max_output_bytes(mut self, maximum: usize) -> Self {
        self.limits = self.limits.with_max_output_bytes(maximum);
        self
    }

    /// Sets the root-inclusive depth maximum shared by both directions.
    pub(super) fn with_max_depth(mut self, maximum: usize) -> Self {
        self.limits = self.limits.with_max_depth(maximum);
        self
    }

    /// Sets the cumulative node maximum shared by both directions.
    pub(super) fn with_max_nodes(mut self, maximum: usize) -> Self {
        self.limits = self.limits.with_max_nodes(maximum);
        self
    }

    /// Sets the per-array item maximum shared by both directions.
    pub(super) fn with_max_sequence_items(mut self, maximum: usize) -> Self {
        self.limits = self.limits.with_max_sequence_items(maximum);
        self
    }

    /// Sets the per-object entry maximum shared by both directions.
    pub(super) fn with_max_map_entries(mut self, maximum: usize) -> Self {
        self.limits = self.limits.with_max_map_entries(maximum);
        self
    }

    /// Sets the per-key UTF-8 byte maximum shared by both directions.
    pub(super) fn with_max_key_bytes(mut self, maximum: usize) -> Self {
        self.limits = self.limits.with_max_key_bytes(maximum);
        self
    }

    /// Sets the per-string UTF-8 byte maximum shared by both directions.
    pub(super) fn with_max_string_bytes(mut self, maximum: usize) -> Self {
        self.limits = self.limits.with_max_string_bytes(maximum);
        self
    }

    /// Sets the per-number lexical byte maximum shared by both directions.
    pub(super) fn with_max_number_bytes(mut self, maximum: usize) -> Self {
        self.limits = self.limits.with_max_number_bytes(maximum);
        self
    }

    /// Sets the cumulative key, string, and number payload maximum.
    pub(super) fn with_max_payload_bytes(mut self, maximum: usize) -> Self {
        self.limits = self.limits.with_max_payload_bytes(maximum);
        self
    }

    /// Creates fresh encode accounting from this test configuration.
    pub(super) fn encode_session(&self) -> JsonEncodeSession<'static> {
        JsonEncodeSession::owned(self.limits)
    }
}
