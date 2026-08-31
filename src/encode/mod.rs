// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Strict JSON encoding APIs and their budget-aware serializer internals.

mod json_collection_kind;
mod json_encode_error;
mod json_encoder;
mod json_integer_signedness;
mod json_map_key_kind;
mod json_serialization_error;
mod json_serialization_error_category;
mod json_serialization_error_kind;
mod json_serializer_state_error;
mod output;
mod serde_compat;
mod serializer;

pub use json_collection_kind::JsonCollectionKind;
pub use json_encode_error::JsonEncodeError;
pub use json_encoder::JsonEncoder;
pub use json_integer_signedness::JsonIntegerSignedness;
pub use json_map_key_kind::JsonMapKeyKind;
pub use json_serialization_error::JsonSerializationError;
pub use json_serialization_error_category::JsonSerializationErrorCategory;
pub use json_serialization_error_kind::JsonSerializationErrorKind;
pub use json_serializer_state_error::JsonSerializerStateError;
