// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Error types returned by the lenient JSON decoder.

mod error_privacy_policy;
mod json_decode_error;
mod json_decode_error_kind;
mod json_decode_stage;

pub use error_privacy_policy::ErrorPrivacyPolicy;
pub use json_decode_error::JsonDecodeError;
pub use json_decode_error_kind::JsonDecodeErrorKind;
pub use json_decode_stage::JsonDecodeStage;
