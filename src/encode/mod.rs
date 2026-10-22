// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Strict JSON encoding APIs and their budget-aware serializer internals.

mod internal;
#[cfg(feature = "jcs")]
mod jcs_encode_error;
#[cfg(feature = "jcs")]
mod jcs_encode_error_kind;
#[cfg(feature = "jcs")]
mod jcs_encoder;
#[cfg(feature = "jcs")]
mod jcs_working_memory_error;
mod json_collection_kind;
mod json_encode_error;
mod json_encode_error_kind;
mod json_encode_error_source;
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

#[cfg(feature = "jcs")]
pub use jcs_encode_error::JcsEncodeError;
#[cfg(feature = "jcs")]
pub use jcs_encode_error_kind::JcsEncodeErrorKind;
#[cfg(feature = "jcs")]
pub use jcs_encoder::JcsEncoder;
#[cfg(feature = "jcs")]
pub use jcs_working_memory_error::JcsWorkingMemoryError;
pub use json_collection_kind::JsonCollectionKind;
pub use json_encode_error::JsonEncodeError;
pub use json_encode_error_kind::JsonEncodeErrorKind;
pub use json_encode_error_source::JsonEncodeErrorSource;
pub use json_encoder::JsonEncoder;
pub use json_integer_signedness::JsonIntegerSignedness;
pub use json_map_key_kind::JsonMapKeyKind;
pub use json_serialization_error::JsonSerializationError;
pub use json_serialization_error_category::JsonSerializationErrorCategory;
pub use json_serialization_error_kind::JsonSerializationErrorKind;
pub use json_serializer_state_error::JsonSerializerStateError;
