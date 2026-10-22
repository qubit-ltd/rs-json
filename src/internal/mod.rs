// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared implementation details used across JSON domains.

mod json_lexeme_length;
mod json_lexeme_length_writer;
mod json_map_key;
mod json_private_protocol_kind;
mod json_private_protocol_phase;
mod json_private_protocol_state;
mod serde_json_compat;
mod strict_json_number;

pub(crate) use json_lexeme_length::JsonLexemeLength;
pub(crate) use json_lexeme_length_writer::JsonLexemeLengthWriter;
pub(crate) use json_map_key::JsonMapKey;
pub(crate) use json_private_protocol_kind::JsonPrivateProtocolKind;
pub(crate) use json_private_protocol_phase::JsonPrivateProtocolPhase;
pub(crate) use json_private_protocol_state::JsonPrivateProtocolState;
#[cfg(test)]
pub(crate) use serde_json_compat::SERDE_JSON_NUMBER_TOKEN;
pub(crate) use serde_json_compat::SERDE_JSON_RAW_VALUE_TOKEN;
pub(crate) use strict_json_number::StrictJsonNumber;
