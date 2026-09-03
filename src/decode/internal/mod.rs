// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private implementation details for lenient JSON normalization.

mod control_character_escaper;
mod decode_metadata;
mod json_decode_engine;
mod json_document_decoder;
pub(super) mod json_normalizer;
mod markdown_fence;
mod typed_seed;

pub(in crate::decode) use decode_metadata::DecodeMetadata;
pub(in crate::decode) use json_decode_engine::JsonDecodeEngine;
pub(super) use json_document_decoder::admit_json_document;
pub(super) use json_document_decoder::deserialize_json_document;
pub(in crate::decode) use json_normalizer::JsonNormalizer;
pub(super) use typed_seed::TypedSeed;
