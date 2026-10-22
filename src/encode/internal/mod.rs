// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Crate-private representation details for JSON encoding errors.

#[cfg(feature = "jcs")]
mod jcs_encode_failure;
mod json_encode_failure;

#[cfg(feature = "jcs")]
mod jcs_renderer;
#[cfg(feature = "jcs")]
mod jcs_renderer_error;
#[cfg(feature = "jcs")]
mod jcs_renderer_frame;
#[cfg(feature = "jcs")]
mod jcs_working_memory;

#[cfg(feature = "jcs")]
pub(in crate::encode) use jcs_encode_failure::JcsEncodeFailure;
#[cfg(feature = "jcs")]
pub(in crate::encode) use jcs_renderer::JcsRenderer;
#[cfg(feature = "jcs")]
pub(in crate::encode) use jcs_renderer_error::JcsRendererError;
pub(in crate::encode) use json_encode_failure::JsonEncodeFailure;
