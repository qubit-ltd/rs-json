// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private implementation details for lenient JSON normalization.

mod control_character_escaper;
mod json_document_decoder;
pub(super) mod json_normalizer;
mod markdown_fence;
mod normalizing_json_decode_failure;
mod typed_seed;

pub(super) use json_document_decoder::admit_json_document;
pub(super) use json_document_decoder::deserialize_json_document;
pub(super) use normalizing_json_decode_failure::NormalizingJsonDecodeFailure;
pub(super) use typed_seed::TypedSeed;
