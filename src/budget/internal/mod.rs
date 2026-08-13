// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private helpers for budget-aware JSON/Serde adapters.

mod json_encode_compound;
mod json_encode_serializer;
mod json_lexical_preflight;
mod json_output_buffer;

pub(in crate::budget) use json_encode_serializer::JsonEncodeSerializer;
pub(crate) use json_lexical_preflight::JsonLexicalPreflight;
pub(in crate::budget) use json_output_buffer::JsonOutputAccounting;
pub(in crate::budget) use json_output_buffer::JsonOutputBuffer;
