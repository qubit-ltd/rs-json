// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Builds [`serde_json::Value`] values while charging a JSON value budget.

mod duplicate_key_rejecting_json_value;
mod json_value_encode_error;
mod json_value_encoder;
mod json_value_seed;

pub use duplicate_key_rejecting_json_value::DuplicateKeyRejectingJsonValue;
pub use duplicate_key_rejecting_json_value::DuplicateKeyRejectingJsonValueSeed;
pub use json_value_encode_error::JsonValueEncodeError;
pub use json_value_encoder::JsonValueEncoder;
pub use json_value_seed::JsonValueSeed;

pub mod traverse;

mod internal;
