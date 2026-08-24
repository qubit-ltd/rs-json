// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Builds [`serde_json::Value`] values while charging a JSON value budget.

mod json_value_seed;
mod strict_json_value;

pub use internal::json_number_lexeme_length;
pub use json_value_seed::JsonValueSeed;
pub use strict_json_value::StrictJsonValue;
pub use strict_json_value::StrictJsonValueSeed;

pub mod traverse;

mod internal;
