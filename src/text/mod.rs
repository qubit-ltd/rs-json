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

pub use json_decode_error::JsonDecodeError;
pub use json_encode_error::JsonEncodeError;
pub use json_syntax_error::JsonSyntaxError;
pub use json_syntax_error_reason::JsonSyntaxErrorReason;
pub use json_text_decoder::JsonTextDecoder;
pub use json_text_encoder::JsonTextEncoder;

mod internal;
