// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tracks one serde_json private compound protocol from construction to end.

use crate::encode::JsonSerializerStateError;
use crate::internal::JsonPrivateProtocolKind;
use crate::internal::JsonPrivateProtocolPhase;

/// State for validating the exact shape of one private protocol compound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JsonPrivateProtocolState {
    /// Protocol whose field and failure contract is being checked.
    kind: JsonPrivateProtocolKind,
    /// Current payload lifecycle phase.
    phase: JsonPrivateProtocolPhase,
}

impl JsonPrivateProtocolState {
    /// Creates a protocol state after validating its declared field count.
    pub(crate) fn new(
        kind: JsonPrivateProtocolKind,
        declared_len: usize,
    ) -> Result<Self, JsonSerializerStateError> {
        if declared_len != 1 {
            return Err(kind.invalid_state());
        }
        Ok(Self {
            kind,
            phase: JsonPrivateProtocolPhase::Pending,
        })
    }

    /// Starts the only permitted payload field.
    pub(crate) fn begin(&mut self, field: &str) -> Result<(), JsonSerializerStateError> {
        if self.phase != JsonPrivateProtocolPhase::Pending || field != self.kind.field_name() {
            self.phase = JsonPrivateProtocolPhase::Failed;
            return Err(self.kind.invalid_state());
        }
        self.phase = JsonPrivateProtocolPhase::InFlight;
        Ok(())
    }

    /// Marks the payload complete or poisons an invalid transition.
    pub(crate) fn complete(&mut self) -> Result<(), JsonSerializerStateError> {
        if self.phase != JsonPrivateProtocolPhase::InFlight {
            self.phase = JsonPrivateProtocolPhase::Failed;
            return Err(self.kind.invalid_state());
        }
        self.phase = JsonPrivateProtocolPhase::Complete;
        Ok(())
    }

    /// Marks the protocol failed after its payload callback returns an error.
    pub(crate) fn fail(&mut self) {
        self.phase = JsonPrivateProtocolPhase::Failed;
    }

    /// Confirms that exactly one payload completed successfully.
    pub(crate) fn finish(self) -> Result<(), JsonSerializerStateError> {
        if self.phase == JsonPrivateProtocolPhase::Complete {
            Ok(())
        } else {
            Err(self.kind.invalid_state())
        }
    }
}
