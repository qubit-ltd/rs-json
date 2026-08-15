// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private implementation of strict JSON text encoding.

mod budgeted_display_collector;
mod budgeted_key;
mod budgeted_private_value;
mod budgeted_value;
mod display_budget_kind;
mod json_encode_compound;
mod json_encode_context;
mod json_encode_serializer;
mod json_lexeme_length;
mod json_output_accounting;
mod json_output_buffer;
mod json_output_writer;
mod private_struct_kind;
mod serde_json_compat;

pub(in crate::text) use json_encode_context::JsonEncodeContext;
pub(in crate::text) use json_encode_serializer::JsonEncodeSerializer;
pub(in crate::text) use json_output_accounting::JsonOutputAccounting;
pub(in crate::text) use json_output_buffer::JsonOutputBuffer;
pub(in crate::text) use json_output_writer::JsonOutputWriter;
