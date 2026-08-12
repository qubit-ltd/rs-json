// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Budget-aware JSON/Serde adapters.

mod budgeted_json_value_seed;
mod internal;
mod json_decode;
mod json_decode_limits;
mod json_decode_session;
mod json_encode;
mod json_encode_limits;
mod json_encode_session;
mod json_resource;
mod json_serde_error;
mod json_syntax_error;
mod json_syntax_error_reason;
mod json_value_accounting;
mod json_value_budget;
mod json_value_limits;
mod json_value_visitor;

pub use budgeted_json_value_seed::BudgetedJsonValueSeed;
pub use json_decode::decode_slice;
pub use json_decode::decode_slice_seed;
pub use json_decode_limits::JsonDecodeLimits;
pub use json_decode_session::JsonDecodeSession;
pub use json_encode::encode_to_vec;
pub use json_encode::encode_to_writer;
pub use json_encode_limits::JsonEncodeLimits;
pub use json_encode_session::JsonEncodeSession;
pub use json_resource::JsonResource;
pub use json_serde_error::JsonSerdeError;
pub use json_syntax_error::JsonSyntaxError;
pub use json_syntax_error_reason::JsonSyntaxErrorReason;
pub use json_value_accounting::account_value;
pub use json_value_budget::JsonValueBudget;
pub use json_value_limits::JsonValueLimits;
pub use json_value_visitor::JsonValueVisitor;
pub use json_value_visitor::walk_json_value;
