// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared crate-private lexical infrastructure.

mod json_lexical_container_frame;
mod json_lexical_cursor;
mod json_lexical_error;
mod json_lexical_error_reason;
mod json_lexical_failure;
mod json_lexical_scanner;

pub(crate) use json_lexical_error::JsonLexicalError;
pub(crate) use json_lexical_error_reason::JsonLexicalErrorReason;
pub(crate) use json_lexical_failure::JsonLexicalFailure;
pub(crate) use json_lexical_scanner::JsonLexicalScanner;
