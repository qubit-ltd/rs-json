// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Budget-aware JSON/Serde adapters.

pub(crate) mod budgeted_json_value_seed;
pub(crate) mod internal;
mod json_decode;
mod json_encode;
mod json_serde_error;
mod json_syntax_error;
mod json_syntax_error_reason;

pub use json_decode::decode_slice;
pub use json_decode::decode_slice_seed;
pub use json_encode::encode_to_vec;
pub use json_encode::encode_to_writer;
pub use json_encode::encode_to_writer_incremental;
pub use json_serde_error::JsonSerdeError;
pub use json_syntax_error::JsonSyntaxError;
pub use json_syntax_error_reason::JsonSyntaxErrorReason;
pub(crate) use qubit_budget::json::JsonDecodeSession;
pub(crate) use qubit_budget::json::JsonEncodeSession;
