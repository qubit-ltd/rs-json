// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Normalizes non-standard JSON text before strict decoding.

mod error_privacy_policy;
mod json_top_level_kind;
mod lenient_json_decode_error;
mod lenient_json_decode_error_kind;
mod lenient_json_decode_options;
mod lenient_json_decode_stage;
mod lenient_json_decoder;
mod markdown_fence_closing;
mod markdown_fence_policy;

pub use error_privacy_policy::ErrorPrivacyPolicy;
pub use json_top_level_kind::JsonTopLevelKind;
pub use lenient_json_decode_error::LenientJsonDecodeError;
pub use lenient_json_decode_error_kind::LenientJsonDecodeErrorKind;
pub use lenient_json_decode_options::LenientJsonDecodeOptions;
pub use lenient_json_decode_stage::LenientJsonDecodeStage;
pub use lenient_json_decoder::LenientJsonDecoder;
pub use markdown_fence_closing::MarkdownFenceClosing;
pub use markdown_fence_policy::MarkdownFencePolicy;

mod internal;
