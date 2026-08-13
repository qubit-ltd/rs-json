// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Normalizes non-standard JSON text before strict decoding.

pub use crate::error::ErrorPrivacyPolicy;
pub use crate::error::JsonDecodeError as LenientJsonDecodeError;
pub use crate::error::JsonDecodeErrorKind;
pub use crate::error::JsonDecodeStage;
pub use crate::json_top_level_kind::JsonTopLevelKind;
pub use crate::lenient_json_decoder::LenientJsonDecoder;
pub use crate::options::JsonDecodeOptions;
pub use crate::options::MarkdownFenceClosing;
pub use crate::options::MarkdownFencePolicy;
