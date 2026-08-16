// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Strict JSON encoding APIs and their budget-aware serializer internals.

mod json_encode_error;
mod json_encoder;
mod output;
mod serde_compat;
mod serializer;

pub use json_encode_error::JsonEncodeError;
pub use json_encoder::JsonEncoder;
