// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private Serde roles used to build budgeted JSON values.

mod json_key_seed;
mod json_key_visitor;
mod json_number_length_writer;
mod json_number_lexeme_length;
mod json_value_child_seed;
mod json_value_frame;
mod json_value_materialization_error;
mod json_value_measurement;
mod json_value_visitor;
mod lexical_json_value_builder;
mod safe_value_builder;

pub(super) use json_key_seed::JsonKeySeed;
pub(crate) use json_number_length_writer::JsonNumberLengthWriter;
pub(crate) use json_number_lexeme_length::json_number_lexeme_length;
pub(super) use json_value_child_seed::JsonValueChildSeed;
pub(crate) use json_value_frame::JsonValueFrame;
pub(crate) use json_value_materialization_error::JsonValueMaterializationError;
pub(crate) use json_value_measurement::json_value_measurement;
pub(super) use json_value_visitor::JsonValueVisitor;
pub(crate) use lexical_json_value_builder::LexicalJsonValueBuilder;
pub(crate) use safe_value_builder::SafeValueBuilder;
