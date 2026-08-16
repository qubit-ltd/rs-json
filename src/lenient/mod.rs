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

/// Configures whether redacted or full errors are exposed.
pub use error_privacy_policy::ErrorPrivacyPolicy;
/// Controls which top-level JSON constructs are allowed in inputs.
pub use json_top_level_kind::JsonTopLevelKind;
/// Lenient decoding failure type carrying normalized input and syntax context.
pub use lenient_json_decode_error::LenientJsonDecodeError;
/// Categorizes the reason of a lenient decoding failure.
pub use lenient_json_decode_error_kind::LenientJsonDecodeErrorKind;
/// Options controlling lenient normalization and admission behavior.
pub use lenient_json_decode_options::LenientJsonDecodeOptions;
/// Processing stage selected for lenient decoding.
pub use lenient_json_decode_stage::LenientJsonDecodeStage;
/// Primary decoder for lenient text decoding workflows.
pub use lenient_json_decoder::LenientJsonDecoder;
/// Closes markdown-style fences while accounting for policy.
pub use markdown_fence_closing::MarkdownFenceClosing;
/// Defines markdown fence behavior for permissive inputs.
pub use markdown_fence_policy::MarkdownFencePolicy;

mod internal;
