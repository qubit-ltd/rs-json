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
mod json_collection_kind;
mod json_integer_signedness;
mod json_map_key_kind;
mod json_serializer_state_error;
mod json_value_encode_error;
mod json_value_encode_error_category;
mod json_value_encode_error_kind;
mod json_value_encoder;

pub use accounting_json_value_seed::AccountingJsonValueSeed;
pub use duplicate_key_rejecting_json_value::DuplicateKeyRejectingJsonValue;
pub use duplicate_key_rejecting_json_value::DuplicateKeyRejectingJsonValueSeed;
pub use json_collection_kind::JsonCollectionKind;
pub use json_integer_signedness::JsonIntegerSignedness;
pub use json_map_key_kind::JsonMapKeyKind;
pub use json_serializer_state_error::JsonSerializerStateError;
pub use json_value_encode_error::JsonValueEncodeError;
pub use json_value_encode_error_category::JsonValueEncodeErrorCategory;
pub use json_value_encode_error_kind::JsonValueEncodeErrorKind;
pub use json_value_encoder::JsonValueEncoder;

pub mod traverse;

mod internal;
