// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private serializer helper types.

mod budgeted_display_collector;
mod budgeted_key;
mod budgeted_private_value;
mod budgeted_value;
mod display_budget_kind;
mod json_encode_compound;
pub(super) mod json_encode_context;
pub(super) mod json_encode_serializer;
mod json_key_budget_serializer;
mod json_private_number_preflight;
mod json_private_text_serializer;
mod private_text_kind;

pub(in crate::encode) use budgeted_display_collector::BudgetedDisplayCollector;
pub(in crate::encode) use budgeted_key::BudgetedKey;
pub(in crate::encode) use budgeted_private_value::BudgetedPrivateValue;
pub(in crate::encode) use budgeted_value::BudgetedValue;
pub(in crate::encode) use display_budget_kind::DisplayBudgetKind;
pub(in crate::encode) use json_encode_compound::JsonEncodeCompound;
pub(in crate::encode) use json_encode_context::JsonEncodeContext;
pub(in crate::encode) use json_encode_serializer::JsonEncodeSerializer;
pub(in crate::encode::serializer) use json_key_budget_serializer::JsonKeyBudgetSerializer;
pub(in crate::encode::serializer) use json_private_text_serializer::JsonPrivateTextSerializer;
pub(in crate::encode::serializer) use private_text_kind::PrivateTextKind;
