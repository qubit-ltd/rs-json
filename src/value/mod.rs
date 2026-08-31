// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Builds [`serde_json::Value`] values while charging a JSON value budget.

mod accounting_json_value_seed;
mod duplicate_key_rejecting_json_value;
mod json_value_encoder;

pub use accounting_json_value_seed::AccountingJsonValueSeed;
pub use duplicate_key_rejecting_json_value::DuplicateKeyRejectingJsonValue;
pub use duplicate_key_rejecting_json_value::DuplicateKeyRejectingJsonValueSeed;
pub use json_value_encoder::JsonValueEncoder;

pub mod traverse;

mod internal;
