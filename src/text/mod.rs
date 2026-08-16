// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Decodes and encodes strict JSON text with explicit resource budgets.

mod json_decode_error;
mod json_encode_error;
mod json_syntax_error;
mod json_syntax_error_reason;
mod json_text_decoder;
mod json_text_encoder;

/// Error type for decode admission, syntax validation, and budgeting failures.
pub use json_decode_error::JsonDecodeError;
/// Error type for strict encoding failures and budget violations.
pub use json_encode_error::JsonEncodeError;
/// Structured error for strict syntax classification.
pub use json_syntax_error::JsonSyntaxError;
/// Root cause codes for strict syntax analysis.
pub use json_syntax_error_reason::JsonSyntaxErrorReason;
/// Strict decoder that enforces session-owned budgets.
pub use json_text_decoder::JsonTextDecoder;
/// Strict encoder that enforces session-owned budgets.
pub use json_text_encoder::JsonTextEncoder;

mod internal;
