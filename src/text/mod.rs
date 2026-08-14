// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Decodes and encodes strict JSON text with explicit resource budgets.

mod json_decode;
mod json_decode_error;
mod json_deserialize_error;
mod json_deserialize_error_category;
mod json_encode;
mod json_encode_error;

pub use json_decode::decode_slice;
pub use json_decode::decode_slice_seed;
pub use json_decode::inspect;
pub use json_decode_error::JsonDecodeError;
pub use json_deserialize_error::JsonDeserializeError;
pub use json_deserialize_error_category::JsonDeserializeErrorCategory;
pub use json_encode::encode_to_vec;
pub use json_encode::encode_to_writer;
pub use json_encode_error::JsonEncodeError;

pub use crate::budget::JsonSyntaxError;
pub use crate::budget::JsonSyntaxErrorReason;
