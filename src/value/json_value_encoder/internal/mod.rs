// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private Serde serializers used by strict JSON value encoding.

mod json_value_compound;
mod json_value_encode_context;
mod json_value_display_collector;
mod json_value_display_kind;
mod json_value_map_key_serializer;
mod json_value_serializer;
mod private_text_serializer;

pub(super) use json_value_compound::JsonValueCompound;
pub(super) use json_value_encode_context::JsonValueEncodeContext;
pub(super) use json_value_encode_context::JsonValueEncodeFailure;
pub(super) use json_value_display_collector::JsonValueDisplayCollector;
pub(super) use json_value_display_kind::JsonValueDisplayKind;
pub(super) use json_value_map_key_serializer::JsonValueMapKeySerializer;
pub(super) use json_value_serializer::JsonValueSerializer;
