// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private lexical cursor and container state.

mod json_lexical_container_frame;
pub(super) mod json_lexical_cursor;

pub(in crate::lexical) use json_lexical_container_frame::JsonLexicalContainerFrame;
