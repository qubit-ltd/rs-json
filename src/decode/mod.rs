// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! JSON decoding APIs and their diagnostics.

mod diagnostic_policy;
mod json_decode_error;
mod json_decode_error_kind;
mod json_decode_error_source;
mod json_decode_stage;
mod json_decoder;
mod json_root_kind;
mod json_syntax_error;
mod json_syntax_error_reason;
mod markdown_fence_closing;
mod markdown_fence_policy;
mod normalized_json_document;
mod normalizing_json_decode_policy;
mod normalizing_json_decode_policy_builder;
mod normalizing_json_decoder;

pub use diagnostic_policy::DiagnosticPolicy;
pub use json_decode_error::JsonDecodeError;
pub use json_decode_error_kind::JsonDecodeErrorKind;
pub use json_decode_error_source::JsonDecodeErrorSource;
pub use json_decode_stage::JsonDecodeStage;
pub use json_decoder::JsonDecoder;
pub use json_root_kind::JsonRootKind;
pub use json_syntax_error::JsonSyntaxError;
pub use json_syntax_error_reason::JsonSyntaxErrorReason;
pub use markdown_fence_closing::MarkdownFenceClosing;
pub use markdown_fence_policy::MarkdownFencePolicy;
pub use normalized_json_document::NormalizedJsonDocument;
pub use normalizing_json_decode_policy::NormalizingJsonDecodePolicy;
pub use normalizing_json_decode_policy_builder::NormalizingJsonDecodePolicyBuilder;
pub use normalizing_json_decoder::NormalizingJsonDecoder;

mod internal;
