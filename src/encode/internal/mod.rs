// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Crate-private representation details for JSON encoding errors.

mod json_encode_failure;

#[cfg(feature = "jcs")]
mod jcs_renderer;
#[cfg(feature = "jcs")]
mod jcs_working_memory;

#[cfg(feature = "jcs")]
pub(in crate::encode) use jcs_renderer::JcsRenderer;
#[cfg(feature = "jcs")]
pub(in crate::encode) use jcs_renderer::JcsRendererError;
pub(in crate::encode) use json_encode_failure::JsonEncodeFailure;
