// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Configuration types for the lenient JSON decoder.

mod json_decode_options;
mod markdown_fence_closing;
mod markdown_fence_policy;

pub use json_decode_options::JsonDecodeOptions;
pub use markdown_fence_closing::MarkdownFenceClosing;
pub use markdown_fence_policy::MarkdownFencePolicy;
