// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private helpers for budget-aware JSON/Serde adapters.

mod budgeted_key;
mod budgeted_private_value;
mod display_budget_kind;
mod json_encode_compound;
mod json_encode_context;
mod json_encode_serializer;
mod json_lexeme_length;
mod json_lexical_preflight;
mod json_output_buffer;
mod json_output_writer;
mod private_struct_kind;
mod serde_json_compat;

pub(in crate::budget) use json_encode_context::JsonEncodeContext;
pub(in crate::budget) use json_encode_serializer::JsonEncodeSerializer;
pub(crate) use json_lexical_preflight::JsonLexicalPreflight;
pub(in crate::budget) use json_output_buffer::JsonOutputAccounting;
pub(in crate::budget) use json_output_buffer::JsonOutputBuffer;
pub(in crate::budget) use json_output_writer::JsonOutputWriter;
