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
mod serde_json_compat;

pub(crate) use json_lexeme_length::JsonLexemeLength;
pub(crate) use json_lexeme_length_writer::JsonLexemeLengthWriter;
pub(crate) use json_map_key::JsonMapKey;
pub(crate) use serde_json_compat::SERDE_JSON_RAW_VALUE_TOKEN;
